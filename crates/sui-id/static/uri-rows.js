// sui-id redirect/post-logout URI row add-remove (RFC 149 Q4 §2).
//
// Without this script, clicking "Add another"/"Remove" submits the
// form to the server, which returns the page again with one row
// more or fewer and saves nothing -- that round trip is the only
// behaviour this script changes: it does the same row add/remove in
// place, so the operator keeps whatever else they had already typed
// without a page reload. The server is still the one that validates
// and saves on an actual "Save" click either way.
//
// No page knowledge: a container holding one field's rows carries
// data-min-rows (how many the server will always keep, mirroring its
// own "at least one" rule for redirect URIs) and data-next-index (the
// next unused index, so inserted rows never collide with one removed
// earlier in the same sitting). Its "Add another" button names that
// container with data-rows-for, plus data-input-name-prefix and
// data-label-template ("Redirect URI {n}") for naming a new row, and
// data-remove-label for its Remove button's text -- the only strings
// this script needs, both already in the page's own language. Each
// row (existing or added) carries data-uri-row; each Remove button,
// data-remove-uri-row.
(function () {
  "use strict";

  function addRow(button) {
    var containerId = button.getAttribute("data-rows-for");
    var container = containerId ? document.getElementById(containerId) : null;
    if (!container) {
      return;
    }
    var prefix = button.getAttribute("data-input-name-prefix") || "";
    var template = button.getAttribute("data-label-template") || "{n}";
    var removeLabel = button.getAttribute("data-remove-label") || "";
    var index = parseInt(container.getAttribute("data-next-index") || "0", 10);
    container.setAttribute("data-next-index", String(index + 1));
    var ordinal = container.querySelectorAll("[data-uri-row]").length + 1;
    var label = template.replace("{n}", String(ordinal));

    var row = document.createElement("div");
    row.className = "uri-row field";
    row.setAttribute("data-uri-row", "");

    var inner = document.createElement("div");
    inner.className = "row gap-1";

    var input = document.createElement("input");
    input.type = "url";
    input.id = prefix + "-" + index;
    input.name = prefix + "_" + index;
    input.setAttribute("aria-label", label);
    inner.appendChild(input);

    var remove = document.createElement("button");
    remove.type = "submit";
    remove.name = "remove_" + prefix;
    remove.value = String(index);
    remove.className = "button secondary";
    remove.setAttribute("data-remove-uri-row", "");
    remove.textContent = removeLabel;
    inner.appendChild(remove);

    row.appendChild(inner);
    container.appendChild(row);
    input.focus();
  }

  function removeRow(button) {
    var row = button.closest("[data-uri-row]");
    var container = row ? row.parentElement : null;
    if (!row || !container) {
      return;
    }
    var minRows = parseInt(container.getAttribute("data-min-rows") || "0", 10);
    var rows = container.querySelectorAll("[data-uri-row]");
    if (rows.length <= minRows) {
      return;
    }
    row.remove();
  }

  document.addEventListener("click", function (ev) {
    var addButton = ev.target.closest("[data-add-uri-row]");
    if (addButton) {
      ev.preventDefault();
      addRow(addButton);
      return;
    }
    var removeButton = ev.target.closest("[data-remove-uri-row]");
    if (removeButton) {
      ev.preventDefault();
      removeRow(removeButton);
    }
  });
})();
