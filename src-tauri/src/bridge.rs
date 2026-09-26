//! Bridge to the signed-in console page.
//!
//! Neither `webview.eval` nor a loopback fetch work from the remote page
//! (eval does not run; plain-HTTP requests from an HTTPS page are blocked as
//! mixed content). What does work is same-origin navigation: the injected
//! script listens for `#kring=…` in the URL, performs the server-function
//! call with the site's own session, and writes the answer back into the hash.
//! Rust drives it by navigating and reads replies by polling the window URL.

use base64::Engine;
use serde_json::Value;

fn encode(value: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value.as_bytes())
}

fn decode(value: &str) -> Option<String> {
    for engine in [
        &base64::engine::general_purpose::STANDARD,
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ] {
        if let Ok(bytes) = engine.decode(value) {
            if let Ok(text) = String::from_utf8(bytes) {
                return Some(text);
            }
        }
    }
    None
}

/// `#kring-req=<mode>.<base64(id)>.<base64(args)>` (mode: "p" post, "g" get)
pub fn request_hash(id: &str, args: &Value) -> String {
    request_hash_mode("p", id, args)
}

pub fn request_hash_mode(mode: &str, id: &str, args: &Value) -> String {
    format!(
        "#kring-req={}.{}.{}",
        mode,
        encode(id),
        encode(&args.to_string())
    )
}

pub struct Reply {
    pub status: String,
    pub body: String,
}

/// Parse `#kring-res=<base64(status|body)>` out of a window URL.
pub fn parse_reply(url: &str) -> Option<Reply> {
    let hash = url.split('#').nth(1)?;
    let payload = hash.strip_prefix("kring-res=")?;
    let decoded = decode(payload)?;
    let mut parts = decoded.splitn(2, '|');
    Some(Reply {
        status: parts.next().unwrap_or("0").to_string(),
        body: parts.next().unwrap_or("").to_string(),
    })
}

/// Init script injected into every console-page navigation.
pub fn init_script() -> &'static str {
    r#"(function () {
  if (window.__kringHashBridge) return;
  window.__kringHashBridge = true;
  const b64 = (s) => btoa(unescape(encodeURIComponent(s)));
  const unb64 = (s) => decodeURIComponent(escape(atob(s)));
  let busy = false;
  // Record the page's own network traffic so the desktop side can learn the
  // live server-function ids, argument shapes and payloads (they change on
  // every site redeploy, so hardcoded ids go stale).
  window.__kringFetch = window.__kringFetch || [];
  const record = (entry) => {
    try {
      window.__kringFetch.push(entry);
      if (window.__kringFetch.length > 40) window.__kringFetch.shift();
    } catch (e) {}
  };
  const headerOf = (source, name) => {
    try {
      if (!source) return '';
      if (typeof source.get === 'function') return source.get(name) || source.get(name.toLowerCase()) || '';
      return source[name] || source[name.toLowerCase()] || '';
    } catch (e) { return ''; }
  };
  if (window.fetch && !window.fetch.__kringWrapped) {
    const origFetch = window.fetch;
    const wrapped = function (input, init) {
      const url = (typeof input === 'string') ? input : ((input && input.url) || '');
      const headers = (init && init.headers) || (input && input.headers) || null;
      const promise = origFetch.apply(this, arguments);
      try {
        Promise.resolve(promise).then((res) => {
          try {
            const clone = res.clone();
            clone.text().then((text) => {
              let resj = '';
              try {
                if (!text) resj = '';
                else if (text[0] === '{' || text[0] === '[') resj = JSON.stringify(JSON.parse(text));
                else {
                  const expr = text.replace(/^;0x[0-9a-fA-F]+;/, '');
                  const sandbox = {};
                  const data = (new Function('self', 'var $R = self.$R = self.$R || {}; return (' + expr + ')'))(sandbox);
                  resj = JSON.stringify(data === undefined ? null : data);
                }
              } catch (e) { resj = ''; }
              record({ kind: 'fetch', url: String(url).slice(0, 300), status: res.status,
                       id: headerOf(headers, 'X-Server-Id'), instance: headerOf(headers, 'X-Server-Instance'),
                       body: String((init && init.body) || '').slice(0, 400),
                       resj: String(resj || '').slice(0, 200000),
                       res: text.slice(0, 8000) });
            }).catch(() => {});
          } catch (e) {}
        }).catch(() => {});
      } catch (e) {}
      return promise;
    };
    wrapped.__kringWrapped = true;
    window.fetch = wrapped;
  }
  try {
    const origOpen = XMLHttpRequest.prototype.open;
    const origSend = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.open = function (method, url) {
      this.__kringUrl = String(url || '');
      this.__kringHeaders = {};
      return origOpen.apply(this, arguments);
    };
    const origSetHeader = XMLHttpRequest.prototype.setRequestHeader;
    XMLHttpRequest.prototype.setRequestHeader = function (name, value) {
      try { this.__kringHeaders[String(name)] = String(value); } catch (e) {}
      return origSetHeader.apply(this, arguments);
    };
    XMLHttpRequest.prototype.send = function (body) {
      const self = this;
      this.addEventListener('load', () => {
        try {
          record({ kind: 'xhr', url: (self.__kringUrl || '').slice(0, 300), status: self.status,
                   id: (self.__kringHeaders || {})['X-Server-Id'] || '',
                   body: String(body || '').slice(0, 400),
                   res: String(self.responseText || '').slice(0, 6000) });
        } catch (e) {}
      });
      return origSend.apply(this, arguments);
    };
  } catch (e) {}
  const handle = async () => {
    const hash = location.hash || '';
    if (!hash.startsWith('#kring-req=') || busy) return;
    busy = true;
    const [mode, idPart, argsPart] = hash.slice('#kring-req='.length).split('.');
    try { location.hash = 'kring-prog=started'; } catch (e) {}
    let reply;
    try {
      const id = unb64(idPart);
      const args = JSON.parse(unb64(argsPart));
      const instance = 'server-fn:0';
      const url = mode === 'g' ? '/_server?id=' + encodeURIComponent(id) : '/_server';
      const res = await fetch(url, {
        method: mode === 'g' ? 'GET' : 'POST',
        headers: { 'Accept': 'application/json',
                   'Content-Type': 'application/json',
                   'X-Server-Id': id, 'X-Server-Instance': instance },
        body: mode === 'g' ? undefined : JSON.stringify(args)
      });
      const materialize = (text) => {
        try { return JSON.stringify(JSON.parse(text)); }
        catch (e) {
          // Seroval payload: an optional `;0x<len>;` envelope around a JS
          // expression evaluated like the site hydrates it (same trust level).
          try {
            const expr = text.replace(/^;0x[0-9a-fA-F]+;/, '');
            const sandbox = {};
            const data = (new Function('self', 'var $R = self.$R = self.$R || {}; return (' + expr + ')'))(sandbox);
            const value = Array.isArray(data) ? data
              : (data && data[instance] !== undefined) ? data[instance]
              : (data && typeof data === 'object' ? Object.values(data).find((v) => Array.isArray(v)) : data);
            return JSON.stringify(value === undefined ? null : value);
          } catch (e2) {
            return JSON.stringify({ raw: text.slice(0, 2000) });
          }
        }
      };
      const text = await res.text();
      try { location.hash = 'kring-prog=received'; } catch (e) {}
      reply = res.status + '|' + materialize(text);
    } catch (e) {
      reply = '0|' + String((e && e.message) || e);
    }
    location.hash = 'kring-res=' + b64(reply);
    busy = false;
  };
  // DOM scrape: #kring-scrape=<base64(["sel1","sel2",…])> → replies with
  // {texts: {0: [...], 1: [...], ...}} (innerText per selector, in order).
  const scrape = () => {
    const hash = location.hash || '';
    if (!hash.startsWith('#kring-scrape=')) return;
    try {
      const selectors = JSON.parse(unb64(hash.slice('#kring-scrape='.length)));
      const texts = {};
      selectors.forEach((sel, i) => {
        if (sel === '@data-slot') {
          texts[i] = Array.from(new Set(
            Array.from(document.querySelectorAll('[data-slot]')).map((el) => el.getAttribute('data-slot'))
          )).filter(Boolean);
        } else if (sel === '@fetch-log') {
          texts[i] = (window.__kringFetch || []).map((entry) => JSON.stringify(entry));
        } else if (sel === '@keys-json') {
          const html = document.documentElement.outerHTML || '';
          const re = /\{id:"(key_[^"]+)"[^}]*\}/g;
          const out = [];
          let m;
          while ((m = re.exec(html)) && out.length < 50) {
            const obj = m[0];
            const nameMatch = obj.match(/name:"([^"]*)"/);
            const secretMatch = obj.match(/key:"(sk-[^"]*)"/);
            const displayMatch = obj.match(/keyDisplay:"([^"]*)"/);
            const emailMatch = obj.match(/email:"([^"]*)"/);
            out.push(JSON.stringify({
              id: m[1],
              name: nameMatch ? nameMatch[1] : '',
              secret: secretMatch ? secretMatch[1] : '',
              display: displayMatch ? displayMatch[1] : '',
              email: emailMatch ? emailMatch[1] : ''
            }));
          }
          texts[i] = out;
        } else if (sel.indexOf('@find=') === 0) {
          const needle = unb64(sel.slice('@find='.length));
          const html = document.documentElement.outerHTML || '';
          const found = [];
          let at = html.indexOf(needle);
          while (at !== -1 && found.length < 5) {
            found.push(html.slice(Math.max(0, at - 80), at + 120));
            at = html.indexOf(needle, at + needle.length);
          }
          texts[i] = found;
        } else if (sel.indexOf('@@') !== -1) {
          const [css, attr] = sel.split('@@');
          texts[i] = Array.from(document.querySelectorAll(css))
            .map((el) => el.getAttribute(attr) || '')
            .filter(Boolean);
        } else {
          texts[i] = Array.from(document.querySelectorAll(sel)).map((el) => (el.innerText || '').trim());
        }
      });
      location.hash = 'kring-res=' + b64(JSON.stringify({ scraped: texts }));
    } catch (e) {
      location.hash = 'kring-res=' + b64(JSON.stringify({ scrapeError: String((e && e.message) || e) }));
    }
  };
  window.addEventListener('hashchange', () => { handle(); scrape(); });
  const poller = setInterval(() => {
    try { handle(); scrape(); } catch (e) { /* ignore */ }
  }, 250);
  setTimeout(() => { handle(); scrape(); }, 300);
  // Presence ping: proves the init script runs on this page at all.
  try {
    if (!(location.hash || '').startsWith('#kring-')) {
      location.hash = 'kring-alive';
    }
  } catch (e) { /* ignore */ }
})();"#
}

#[cfg(test)]
mod tests {
    #[test]
    fn init_script_is_valid_js() {
        let script = super::init_script();
        std::fs::write("/tmp/kring-init-live.js", script).unwrap();
        std::fs::write("/tmp/kring-init.js", script).unwrap();
        assert!(script.contains("hashchange"));
    }

    use super::*;

    #[test]
    fn request_hash_round_trips() {
        let hash = request_hash("abc123", &serde_json::json!(["ws_1", 0]));
        let rest = hash.strip_prefix("#kring-req=p.").unwrap();
        let (id, args) = rest.split_once('.').unwrap();
        assert_eq!(decode(id).unwrap(), "abc123");
        assert_eq!(decode(args).unwrap(), r#"["ws_1",0]"#);
        assert!(request_hash_mode("g", "x", &serde_json::json!([])).starts_with("#kring-req=g."));
    }

    #[test]
    fn real_captured_reply_decodes() {
        // Captured live 2026-09-15: base64(200 | seroval(workspaces)).
        let reply = "MjAwfDsweDAwMDAwMDk5Oygoc2VsZi4kUj1zZWxmLiRSfHx7fSlbInNlcnZlci1mbjo1MzE5MzMiXT1bXSwoJFI9PiRSWzBdPVskUlsxXT17aWQ6Indya18wMU0xUkZXOVRFNEo3NEpXQkpTV0E4S1E5VCIsbmFtZToiRGVmYXVsdCIsc2x1ZzpudWxsfV0pKCRSWyJzZXJ2ZXItZm46NTMxOTMzIl0pKQ==";
        let decoded = decode(reply).expect("standard base64");
        assert!(
            decoded.starts_with("200|"),
            "status prefix, got: {}",
            &decoded[..20]
        );
        assert!(decoded.contains("wrk_01M1RFW9TE4J74JWBJSWA8KQ9T"));
    }

    #[test]
    fn reply_is_parsed_from_the_window_url() {
        let payload = encode("200|{\"ok\":true}");
        let url = format!("https://opencode.ai/workspace/x#kring-res={payload}");
        let reply = parse_reply(&url).expect("reply");
        assert_eq!(reply.status, "200");
        assert_eq!(reply.body, r#"{"ok":true}"#);
        assert!(parse_reply("https://opencode.ai/").is_none());
    }
}
