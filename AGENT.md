I am making a proxy tool that captures traffic from a self-initiated chromium browser. It is very similar to burp suite. 
It has to capture not only packets, but also the contents of the HTTP request/response. 
You have to write a rust program that is separated by modules. This means you must not write more than 150 lines on one file. This will ensure the maintainability of the code. 

The scenario goes like this:
- The user hits `cargo run`.
- Program opens a new browser. 
- User browses like ordinary and quits. 
- Program saves all network traffic on a folder. 

First, tell me your thoughts & plan on this project. Then I will approve your actions. 


  - Best fit is to launch a local Chromium/Chrome with --remote-debugging-port and connect to its DevTools
    websocket. That lets us subscribe to Network.* events and capture full request/response bodies without MITM
    certificates.
  - We’ll save traffic as JSON/NDJSON or HAR-like entries to a dedicated folder for each run. Rust async
    (Tokio) + websockets is a good fit.
  - We must keep files <150 lines, so we’ll split into small modules: config (paths/port), launcher (spawn
    chromium), cdp client (connect + subscribe), recorder (assemble events + write to disk), and main (wiring).

  Plan:

  1. Define config: output directory per run (timestamped), chromium path/flags (defaults, overridable), remote
     debugging port.
  2. Implement launcher module: spawn Chromium with provided port and user data dir, ensure it stays open until
     user quits browser; handle process cleanup.
  3. Implement cdp_client: connect via tokio_tungstenite to ws://127.0.0.1:port/devtools/browser/..., enable
     Network domain, listen for request/response events, fetch bodies with Network.getResponseBody.
  4. Implement recorder: correlate requestId to request/response, serialize to disk (e.g., NDJSON files per
     request or a rolling log) in the run’s folder; include headers, method, URL, status, timing.
  5. main: orchestrate setup, launch browser, start listener, wait until websocket closes or process exits,
     then flush/close files.

  Questions/assumptions:

  - Is Chromium/Chrome already installed? If so, what binary path should we default to (e.g., chromium, google-
    chrome, chrome)?
  - Preference for output format? NDJSON per run is simple; HAR is possible but slightly more work.

1. Assume that I have no chromium binary. Install the binary inside the project.
2. It doesn't matter. Just make the code that make sense.

• Key approach:

  - Launch a headful Chromium with --remote-debugging-port, connect via DevTools WS, subscribe to Network.*,
    and pull bodies with Network.getResponseBody. No MITM/certs needed.
  - Persist traffic as NDJSON/HAR-like entries per run in captures/run-<timestamp>/traffic.ndjson (request,
    response, headers, bodies, timing). Keep files <150 lines by splitting modules.

  Planned modules (each <150 lines):

  - config: resolve output dir, pick port, locate/create browser binary path, user data dir.
  - chromium: ensure/download Chromium into ./binaries/chromium (use chromiumoxide_fetcher), then spawn it
    headful with remote-debug flags; manage lifetime.
  - devtools: fetch WS URL from http://127.0.0.1:<port>/json/version, connect via tokio-tungstenite, enable
    Network domain, stream events, fetch bodies.
  - recorder: correlate requestId lifecycle, serialize to NDJSON, handle flush/cleanup.
  - main: wire everything, run until browser exits/WS closes.

  Dependencies (Cargo): tokio, tokio-tungstenite, serde/serde_json, chromiumoxide_cdp (types),
  chromiumoxide_fetcher (download), anyhow, chrono.

  Needed approval: downloading Chromium (~100–150MB) to ./binaries/chromium via chromiumoxide_fetcher (network
  call). OK to proceed with this plan and download?
