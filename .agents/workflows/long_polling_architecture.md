---
description: Google Messages Long-Polling Architecture & Network Handling
---

# Long-Polling Architecture in gmessages_qt

**CRITICAL INSTRUCTION FOR ALL FUTURE AGENTS:**
Google Messages relies on a *single* long-polling HTTP stream (`receive.bugle.google.com`) per user session. All real-time incoming events (texts) AND asynchronous responses to HTTP RPC requests (like `ListMessages`, `SendMessage`, etc.) are pushed down this *exact same stream*. 

### The Pitfall
Because of this architecture, **you must never spawn multiple active long-polling loops**. 
If you instantiate an ephemeral `SessionHandler` and attach a new long-polling loop to it every time you make a server request, those parallel background loops will violently clash over the singleton session ID. Google will stream the RPC responses round-robin style, which means the background listener waiting for a chat history might have its JSON block stolen by an ambient real-time message listener. This will cause requests to hang endlessly for ~6-10 seconds until they timeout.

### The Correct Implementation
1. **Singleton Controller:** The application has a single continuous stream listener initialized by the `SessionControllerRust` (inside `src/app_state/session_controller.rs`). This listener pulls down all data chunks from Google indefinitely.
2. **Singleton Handler:** Always retrieve the **global session handler** via `make_handler(&client)` instead of instantiating new local handlers. `SharedSession` caches an `RwLock<Option<SessionHandler>>` ensuring everything proxies through exactly one handler.
3. **Yielding Data:** When a network component like `MessageList` executes `send_request`, the raw network bytes are transmitted upstream via the handler. When the response comes back from Google down the *global long-polling stream*, the `SessionController` automatically identifies it as an RPC response stream and calls `handler.process_payload(data).await`. This surgically untangles the RPC buffer, matches the `response_id` back to your originating request awaiting in `send_request`, and seamlessly fulfills the Future.

**NEVER CREATE OR SPAWN NEW `start_handler_loop` WORKERS WHEN LOADING CHATS OR SENDING MESSAGES.**
If you're making an API call, just use `send_request` or `send_request_with_timeout` on the `make_handler` singleton. The background `SessionController` logic already inherently routes the response block directly back to your waiting thread.
