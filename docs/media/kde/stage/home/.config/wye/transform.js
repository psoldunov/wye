// Return a new URL to change the link, or nothing to keep it.
//
// url      The link after expansion and cleaning, as a WHATWG URL.
// context  { sourceApp, entryPoint, heldKeys, rule }
export default function transform(url, context) {
  // Read Reddit threads on the old, lighter site.
  if (url.hostname === "www.reddit.com") {
    url.hostname = "old.reddit.com";
  }

  // Open Google AMP pages as the article they wrap.
  const amp = "/amp/s/";
  if (url.hostname === "www.google.com" && url.pathname.startsWith(amp)) {
    return "https://" + url.pathname.slice(amp.length);
  }

  // Wikipedia links from the chat app open in English.
  if (context.sourceApp === "org.kde.neochat.desktop") {
    url.hostname = url.hostname.replace(/^\w+\.(?=wikipedia)/, "en.");
  }

  return url;
}
