// The globals a transform script sees (SCR-20): `URL`, `URLSearchParams` and
// `console`. Evaluated once per run as a plain script; the value is a
// function the engine calls with the native helpers, so none of them leak
// into the global scope. Parsing, serialising and every setter go through
// Rust's `url` crate (`url::quirks` implements the WHATWG URL API).
(function (natives) {
  "use strict";

  const { parse, parts, update, formParse, formSerialize, log } = natives;

  // Positions in the array `parts` returns.
  const HREF = 0;
  const ORIGIN = 1;
  const PROTOCOL = 2;
  const USERNAME = 3;
  const PASSWORD = 4;
  const HOST = 5;
  const HOSTNAME = 6;
  const PORT = 7;
  const PATHNAME = 8;
  const SEARCH = 9;
  const HASH = 10;

  let linkParams;
  let resetParams;

  const withoutQuestionMark = (text) => (text.startsWith("?") ? text.slice(1) : text);

  class URLSearchParams {
    #list = [];
    #changed = null;

    constructor(init) {
      if (init === undefined || init === null) {
        return;
      }
      if (typeof init === "object" && typeof init[Symbol.iterator] === "function") {
        for (const entry of init) {
          const pair = [...entry];
          if (pair.length !== 2) {
            throw new TypeError("Each URLSearchParams pair must have exactly two values");
          }
          this.#list.push([String(pair[0]), String(pair[1])]);
        }
      } else if (typeof init === "object") {
        for (const key of Object.keys(init)) {
          this.#list.push([key, String(init[key])]);
        }
      } else {
        this.#list = formParse(withoutQuestionMark(String(init)));
      }
    }

    #notify() {
      if (this.#changed !== null) {
        this.#changed(this.#list.length === 0 ? "" : formSerialize(this.#list));
      }
    }

    get size() {
      return this.#list.length;
    }

    append(name, value) {
      this.#list = [...this.#list, [String(name), String(value)]];
      this.#notify();
    }

    delete(name, value) {
      const key = String(name);
      const only = value === undefined ? undefined : String(value);
      this.#list = this.#list.filter(([k, v]) => !(k === key && (only === undefined || v === only)));
      this.#notify();
    }

    get(name) {
      const key = String(name);
      const found = this.#list.find(([k]) => k === key);
      return found === undefined ? null : found[1];
    }

    getAll(name) {
      const key = String(name);
      return this.#list.filter(([k]) => k === key).map(([, v]) => v);
    }

    has(name, value) {
      const key = String(name);
      const only = value === undefined ? undefined : String(value);
      return this.#list.some(([k, v]) => k === key && (only === undefined || v === only));
    }

    set(name, value) {
      const key = String(name);
      const pair = [key, String(value)];
      const first = this.#list.findIndex(([k]) => k === key);
      this.#list =
        first < 0
          ? [...this.#list, pair]
          : this.#list
              .map((entry, index) => (index === first ? pair : entry))
              .filter(([k], index) => index <= first || k !== key);
      this.#notify();
    }

    sort() {
      // Stable, by UTF-16 code units of the name, as WHATWG asks.
      this.#list = this.#list
        .map((entry, index) => [entry, index])
        .sort(([a, i], [b, j]) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : i - j))
        .map(([entry]) => entry);
      this.#notify();
    }

    forEach(callback, thisArg) {
      for (const [k, v] of this.#list) {
        callback.call(thisArg, v, k, this);
      }
    }

    *entries() {
      for (const [k, v] of this.#list) {
        yield [k, v];
      }
    }

    *keys() {
      for (const [k] of this.#list) {
        yield k;
      }
    }

    *values() {
      for (const [, v] of this.#list) {
        yield v;
      }
    }

    [Symbol.iterator]() {
      return this.entries();
    }

    toString() {
      return formSerialize(this.#list);
    }

    static {
      linkParams = (params, changed) => {
        params.#changed = changed;
      };
      resetParams = (params, search) => {
        params.#list = formParse(withoutQuestionMark(search));
      };
    }
  }

  class URL {
    #href;
    #params;

    constructor(input, base) {
      const href =
        base === undefined ? parse(String(input)) : parse(String(input), String(base));
      // The natives answer `undefined` for "not a URL".
      if (href == null) {
        throw new TypeError(`Invalid URL: ${String(input)}`);
      }
      this.#href = href;
      this.#params = new URLSearchParams(parts(href)[SEARCH]);
      linkParams(this.#params, (query) => {
        this.#href = update(this.#href, "search", query);
      });
    }

    static canParse(input, base) {
      return URL.parse(input, base) !== null;
    }

    static parse(input, base) {
      try {
        return new URL(input, base);
      } catch {
        return null;
      }
    }

    #part(index) {
      return parts(this.#href)[index];
    }

    #set(name, value) {
      const next = update(this.#href, name, String(value));
      if (next == null) {
        throw new TypeError(`Invalid URL: ${String(value)}`);
      }
      this.#href = next;
      if (name === "href" || name === "search") {
        resetParams(this.#params, this.#part(SEARCH));
      }
    }

    get href() {
      return this.#href;
    }
    set href(value) {
      this.#set("href", value);
    }
    get origin() {
      return this.#part(ORIGIN);
    }
    get protocol() {
      return this.#part(PROTOCOL);
    }
    set protocol(value) {
      this.#set("protocol", value);
    }
    get username() {
      return this.#part(USERNAME);
    }
    set username(value) {
      this.#set("username", value);
    }
    get password() {
      return this.#part(PASSWORD);
    }
    set password(value) {
      this.#set("password", value);
    }
    get host() {
      return this.#part(HOST);
    }
    set host(value) {
      this.#set("host", value);
    }
    get hostname() {
      return this.#part(HOSTNAME);
    }
    set hostname(value) {
      this.#set("hostname", value);
    }
    get port() {
      return this.#part(PORT);
    }
    set port(value) {
      this.#set("port", value);
    }
    get pathname() {
      return this.#part(PATHNAME);
    }
    set pathname(value) {
      this.#set("pathname", value);
    }
    get search() {
      return this.#part(SEARCH);
    }
    set search(value) {
      this.#set("search", value);
    }
    get hash() {
      return this.#part(HASH);
    }
    set hash(value) {
      this.#set("hash", value);
    }
    get searchParams() {
      return this.#params;
    }

    toString() {
      return this.#href;
    }

    toJSON() {
      return this.#href;
    }
  }

  const show = (value) => {
    if (typeof value === "string") {
      return value;
    }
    if (value instanceof URL || value instanceof URLSearchParams || value instanceof Error) {
      return String(value);
    }
    if (typeof value === "function") {
      return `[function ${value.name || "anonymous"}]`;
    }
    if (typeof value === "object" && value !== null) {
      try {
        return JSON.stringify(value);
      } catch {
        return String(value);
      }
    }
    return String(value);
  };
  const write = (...values) => {
    log(values.map(show).join(" "));
  };

  globalThis.URL = URL;
  globalThis.URLSearchParams = URLSearchParams;
  globalThis.console = Object.freeze({
    log: write,
    info: write,
    warn: write,
    error: write,
    debug: write,
  });

  // What the engine needs, kept out of reach of a script that replaces
  // `globalThis.URL`.
  return {
    make: (href) => new URL(href),
    hrefOf: (value) => (value instanceof URL ? value.href : undefined),
  };
});
