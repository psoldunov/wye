// Return a new URL to change the link, or nothing to keep it.
//
// url      The link after expansion and cleaning, as a WHATWG URL. Mutable.
// context  { sourceApp, entryPoint, heldKeys, rule }
export default function transform(url, context) {
  // Example: open Twitter links on x.com.
  // if (url.hostname === "twitter.com") url.hostname = "x.com";
  return url;
}
