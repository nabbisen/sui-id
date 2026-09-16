// sui-id WebAuthn helpers.
//
// Two flows attached to two specific forms:
//
//   #passkey-register-form  → POST /me/security/passkeys/register/start
//                            → navigator.credentials.create()
//                            → POST /me/security/passkeys/register/complete
//
//   #passkey-auth-form      → POST /admin/login/webauthn/start
//                            → navigator.credentials.get()
//                            → POST /admin/login/webauthn/complete
//
// Encoding helpers: WebAuthn JSON uses base64url-no-pad for byte fields,
// but the JS API hands us ArrayBuffers. We do the dance here.

(function () {
  "use strict";

  function b64urlToBytes(s) {
    s = s.replace(/-/g, "+").replace(/_/g, "/");
    while (s.length % 4) s += "=";
    var raw = atob(s);
    var arr = new Uint8Array(raw.length);
    for (var i = 0; i < raw.length; i++) arr[i] = raw.charCodeAt(i);
    return arr.buffer;
  }

  function bytesToB64url(buf) {
    var bytes = new Uint8Array(buf);
    var s = "";
    for (var i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
    return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  }

  // Walk a CreationChallengeResponse / RequestChallengeResponse JSON
  // value and substitute base64url strings for ArrayBuffers in the
  // places the WebAuthn spec calls for it.
  function decodeCreationOptions(opts) {
    opts.publicKey.challenge = b64urlToBytes(opts.publicKey.challenge);
    opts.publicKey.user.id = b64urlToBytes(opts.publicKey.user.id);
    if (Array.isArray(opts.publicKey.excludeCredentials)) {
      opts.publicKey.excludeCredentials = opts.publicKey.excludeCredentials.map(function (c) {
        return Object.assign({}, c, { id: b64urlToBytes(c.id) });
      });
    }
    return opts.publicKey;
  }

  function decodeRequestOptions(opts) {
    opts.publicKey.challenge = b64urlToBytes(opts.publicKey.challenge);
    if (Array.isArray(opts.publicKey.allowCredentials)) {
      opts.publicKey.allowCredentials = opts.publicKey.allowCredentials.map(function (c) {
        return Object.assign({}, c, { id: b64urlToBytes(c.id) });
      });
    }
    return opts.publicKey;
  }

  function encodeRegistrationCredential(cred) {
    return {
      id: cred.id,
      rawId: bytesToB64url(cred.rawId),
      type: cred.type,
      response: {
        attestationObject: bytesToB64url(cred.response.attestationObject),
        clientDataJSON: bytesToB64url(cred.response.clientDataJSON),
      },
      extensions: cred.getClientExtensionResults
        ? cred.getClientExtensionResults()
        : {},
    };
  }

  function encodeAuthenticationCredential(cred) {
    return {
      id: cred.id,
      rawId: bytesToB64url(cred.rawId),
      type: cred.type,
      response: {
        authenticatorData: bytesToB64url(cred.response.authenticatorData),
        clientDataJSON: bytesToB64url(cred.response.clientDataJSON),
        signature: bytesToB64url(cred.response.signature),
        userHandle: cred.response.userHandle
          ? bytesToB64url(cred.response.userHandle)
          : null,
      },
      extensions: cred.getClientExtensionResults
        ? cred.getClientExtensionResults()
        : {},
    };
  }

  function csrfFromForm(form) {
    var input = form.querySelector('input[name="_csrf"]');
    return input ? input.value : "";
  }

  function postForm(url, body) {
    return fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      credentials: "same-origin",
      body: body,
      redirect: "manual",
    });
  }

  // ---------- registration ----------
  var regForm = document.getElementById("passkey-register-form");
  if (regForm) {
    regForm.addEventListener("submit", function (e) {
      e.preventDefault();
      var csrf = csrfFromForm(regForm);
      var nickname = regForm.querySelector('input[name="nickname"]').value;
      var body = "_csrf=" + encodeURIComponent(csrf) +
                 "&nickname=" + encodeURIComponent(nickname);
      // RFC 102 B7: a user with no second factor re-enters the password.
      var pw = regForm.querySelector('input[name="current_password"]');
      if (pw) {
        body += "&current_password=" + encodeURIComponent(pw.value);
      }
      postForm("/me/security/passkeys/register/start", body)
        .then(function (r) {
          // A user who already has a factor is redirected to step-up; with
          // redirect:'manual' that arrives as an opaque redirect.
          if (r.type === "opaqueredirect") {
            window.location.href =
              "/me/security/step-up?return_to=%2Fme%2Fsecurity%2Fpasskeys";
            throw new Error("step-up required");
          }
          if (!r.ok) {
            return r.json().then(
              function (j) {
                throw new Error((j && j.message) || "server rejected start");
              },
              function () {
                throw new Error("server rejected start");
              }
            );
          }
          return r.json();
        })
        .then(function (opts) {
          return navigator.credentials.create({
            publicKey: decodeCreationOptions(opts),
          });
        })
        .then(function (cred) {
          var enc = encodeRegistrationCredential(cred);
          var completeBody = "_csrf=" + encodeURIComponent(csrf) +
            "&credential=" + encodeURIComponent(JSON.stringify(enc));
          return postForm("/me/security/passkeys/register/complete", completeBody);
        })
        .then(function (r) {
          // The server returns a redirect (303). fetch with redirect:'manual'
          // surfaces this as an opaqueredirect; the simplest thing to do
          // is reload the profile page so the user sees the new passkey.
          window.location.href = "/me/security/passkeys";
        })
        .catch(function (err) {
          if (err && err.message === "step-up required") return;
          console.error("passkey registration failed", err);
          alert("Passkey registration failed: " + (err && err.message ? err.message : err));
        });
    });
  }

  // ---------- authentication ----------
  var authForm = document.getElementById("passkey-auth-form");
  if (authForm) {
    authForm.addEventListener("submit", function (e) {
      e.preventDefault();
      var csrf = csrfFromForm(authForm);
      var body = "_csrf=" + encodeURIComponent(csrf);
      postForm("/admin/login/webauthn/start", body)
        .then(function (r) {
          if (!r.ok) throw new Error("server rejected start");
          return r.json();
        })
        .then(function (opts) {
          return navigator.credentials.get({
            publicKey: decodeRequestOptions(opts),
          });
        })
        .then(function (cred) {
          var enc = encodeAuthenticationCredential(cred);
          var completeBody = "_csrf=" + encodeURIComponent(csrf) +
            "&credential=" + encodeURIComponent(JSON.stringify(enc));
          return postForm("/admin/login/webauthn/complete", completeBody);
        })
        .then(function (r) {
          window.location.href = "/admin";
        })
        .catch(function (err) {
          console.error("passkey login failed", err);
          alert("Sign-in with passkey failed: " + (err && err.message ? err.message : err));
        });
    });
  }
})();
