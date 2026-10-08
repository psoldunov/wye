#!/usr/bin/env bash
# Submit a released version of the browser extension to its store.
#
#   frontends/extension/store/submit.sh amo ZIP VERSION
#   frontends/extension/store/submit.sh chrome ZIP VERSION
#
# amo uploads the Firefox zip to addons.mozilla.org as a new listed version
# of wye@soldunov.dev, with release notes that link the GitHub release.
# Environment: AMO_JWT_ISSUER and AMO_JWT_SECRET, the API credentials from
# https://addons.mozilla.org/developers/addon/api/key/.
#
# chrome uploads the Chromium webstore zip (the manifest without `key`) to
# the Chrome Web Store item jdcifhpoallkdjnbflfienpboodjfjei and submits it
# for review. Environment: CWS_SERVICE_ACCOUNT_KEY, the JSON key of the
# service account added under Account in the Developer Dashboard, and
# CWS_PUBLISHER_ID (Publisher > Settings).
#
# Both stores review the version and publish it once the review passes; this
# returns as soon as the store has accepted the submission. A version the
# store already has is left alone, so running it twice is safe. Needs curl,
# jq and openssl. Called by .github/workflows/stores.yml.
set -euo pipefail

readonly AMO_API=https://addons.mozilla.org/api/v5
readonly AMO_ADDON=wye%40soldunov.dev
readonly CWS_API=https://chromewebstore.googleapis.com
readonly CWS_ITEM=jdcifhpoallkdjnbflfienpboodjfjei
readonly CWS_SCOPE=https://www.googleapis.com/auth/chromewebstore
readonly GOOGLE_TOKEN_URL=https://oauth2.googleapis.com/token
readonly RELEASES=https://github.com/psoldunov/wye/releases/tag
readonly TEST_INSTRUCTIONS=https://github.com/psoldunov/wye/blob/master/frontends/extension/store/README.md#chrome-web-store
readonly POLL_SECONDS=10
readonly POLL_TRIES=60

die() {
  echo "submit.sh: $*" >&2
  exit 1
}

base64url() {
  openssl base64 -A | tr '+/' '-_' | tr -d '='
}

# request CURL_ARGS… URL: the response body on stdout. A status other than
# 2xx prints the body on stderr and fails. The URL comes last.
request() {
  local body status url=${*: -1}
  body=$(mktemp)
  if ! status=$(curl -sS -o "$body" -w '%{http_code}' "$@"); then
    rm -f "$body"
    die "request to $url failed"
  fi
  if [ "${status:0:1}" != 2 ]; then
    cat "$body" >&2
    echo >&2
    rm -f "$body"
    die "HTTP $status from $url"
  fi
  cat "$body"
  rm -f "$body"
}

# The JWT the AMO API takes: HS256, a fresh jti, valid for 60 seconds.
amo_token() {
  local now header claims signature
  now=$(date +%s)
  header=$(printf '{"alg":"HS256","typ":"JWT"}' | base64url)
  claims=$(jq -cn --arg iss "$AMO_JWT_ISSUER" --arg jti "$(openssl rand -hex 16)" \
    --argjson iat "$now" '{iss: $iss, jti: $jti, iat: $iat, exp: ($iat + 60)}' | base64url)
  signature=$(printf '%s.%s' "$header" "$claims" |
    openssl dgst -sha256 -hmac "$AMO_JWT_SECRET" -binary | base64url)
  printf '%s.%s.%s' "$header" "$claims" "$signature"
}

amo() {
  local zip=$1 version=$2 versions upload uuid detail=
  : "${AMO_JWT_ISSUER:?}" "${AMO_JWT_SECRET:?}"

  # Every listed version, also those awaiting review.
  versions=$(request -H "Authorization: JWT $(amo_token)" \
    "$AMO_API/addons/addon/$AMO_ADDON/versions/?filter=all_without_unlisted&page_size=50")
  if jq -e --arg version "$version" '.results[].version | select(. == $version)' \
    <<< "$versions" > /dev/null; then
    echo "addons.mozilla.org already has $version"
    return
  fi

  upload=$(request -H "Authorization: JWT $(amo_token)" \
    -F "upload=@$zip" -F channel=listed "$AMO_API/addons/upload/")
  uuid=$(jq -er .uuid <<< "$upload")
  echo "uploaded $zip as $uuid; waiting for validation"
  for _ in $(seq "$POLL_TRIES"); do
    detail=$(request -H "Authorization: JWT $(amo_token)" "$AMO_API/addons/upload/$uuid/")
    if [ "$(jq -r .processed <<< "$detail")" = true ]; then
      break
    fi
    sleep "$POLL_SECONDS"
  done
  if [ "$(jq -r .processed <<< "$detail")" != true ]; then
    die "addons.mozilla.org did not validate upload $uuid in time"
  fi
  if [ "$(jq -r .valid <<< "$detail")" != true ]; then
    jq '.validation.messages[]? | select(.type == "error") | {message, file}' <<< "$detail" >&2
    die "addons.mozilla.org rejected $zip"
  fi
  if [ "$(jq -r .version <<< "$detail")" != "$version" ]; then
    die "$zip is version $(jq -r .version <<< "$detail"), not $version"
  fi

  jq -n --arg upload "$uuid" \
    --arg notes "Wye $version: $RELEASES/v$version" \
    --arg approval "The companion of Wye, a browser picker for Linux. Without Wye the toolbar popup explains that its helper is missing. Test instructions (Firefox for Chrome): $TEST_INSTRUCTIONS" \
    '{upload: $upload, release_notes: {"en-US": $notes}, approval_notes: $approval}' |
    request -H "Authorization: JWT $(amo_token)" -H 'Content-Type: application/json' \
      --data-binary @- "$AMO_API/addons/addon/$AMO_ADDON/versions/" |
    jq -c '{version, channel, file: .file.status}'
  echo "submitted $version to addons.mozilla.org for review"
}

# An access token for the service account: a JWT signed with its key,
# exchanged at Google's token endpoint.
cws_token() {
  local now header claims signature
  now=$(date +%s)
  header=$(printf '{"alg":"RS256","typ":"JWT"}' | base64url)
  claims=$(jq -c --arg scope "$CWS_SCOPE" --arg aud "$GOOGLE_TOKEN_URL" --argjson iat "$now" \
    '{iss: .client_email, scope: $scope, aud: $aud, iat: $iat, exp: ($iat + 600)}' \
    <<< "$CWS_SERVICE_ACCOUNT_KEY" | base64url)
  signature=$(printf '%s.%s' "$header" "$claims" |
    openssl dgst -sha256 -binary -sign <(jq -r .private_key <<< "$CWS_SERVICE_ACCOUNT_KEY") |
    base64url)
  request --data-urlencode grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer \
    --data-urlencode "assertion=$header.$claims.$signature" "$GOOGLE_TOKEN_URL" |
    jq -er .access_token
}

chrome() {
  local zip=$1 version=$2 token item current upload state=
  : "${CWS_SERVICE_ACCOUNT_KEY:?}" "${CWS_PUBLISHER_ID:?}"
  token=$(cws_token)
  item=publishers/$CWS_PUBLISHER_ID/items/$CWS_ITEM

  current=$(request -H "Authorization: Bearer $token" "$CWS_API/v2/$item:fetchStatus")
  if jq -e --arg version "$version" \
    '[.publishedItemRevisionStatus, .submittedItemRevisionStatus][]?.distributionChannels[]?.crxVersion | select(. == $version)' \
    <<< "$current" > /dev/null; then
    echo "the Chrome Web Store already has $version"
    return
  fi

  upload=$(request -X POST -H "Authorization: Bearer $token" -T "$zip" "$CWS_API/upload/v2/$item:upload")
  state=$(jq -r .uploadState <<< "$upload")
  for _ in $(seq "$POLL_TRIES"); do
    if [ "$state" != IN_PROGRESS ]; then
      break
    fi
    sleep "$POLL_SECONDS"
    state=$(request -H "Authorization: Bearer $token" "$CWS_API/v2/$item:fetchStatus" |
      jq -r .lastAsyncUploadState)
  done
  if [ "$state" != SUCCEEDED ]; then
    echo "$upload" >&2
    die "the Chrome Web Store upload of $zip ended in $state"
  fi
  # crxVersion is only set when the upload finished synchronously.
  if jq -e --arg version "$version" '.crxVersion // $version | . != $version' <<< "$upload" > /dev/null; then
    die "$zip is version $(jq -r .crxVersion <<< "$upload"), not $version"
  fi
  echo "uploaded $zip"

  request -X POST -H "Authorization: Bearer $token" -H 'Content-Type: application/json' -d '{}' \
    "$CWS_API/v2/$item:publish" | jq -c '{state, warningInfo}'
  echo "submitted $version to the Chrome Web Store for review"
}

if [ "$#" -ne 3 ]; then
  die "usage: submit.sh amo|chrome ZIP VERSION"
fi
store=$1 zip=$2 version=$3
if [ ! -f "$zip" ]; then
  die "no file $zip"
fi
if ! [[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  die "version $version is not X.Y.Z"
fi
case $store in
  amo) amo "$zip" "$version" ;;
  chrome) chrome "$zip" "$version" ;;
  *) die "unknown store $store; use amo or chrome" ;;
esac
