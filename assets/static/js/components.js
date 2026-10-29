// Server-rendered "components" (the idea behind Laravel Volt/Livewire, without a framework).
// A <form data-component> inside an element with data-component-root is posted with fetch and the
// server answers with the component's new HTML, which replaces the old one in place. No page load.
// Without JavaScript the form still posts normally and the server returns the whole page.
(function () {
  document.addEventListener('submit', async function (event) {
    var form = event.target.closest('form[data-component]');
    if (!form) return;
    event.preventDefault();
    var root = form.closest('[data-component-root]') || form;
    var button = form.querySelector('[type=submit]');
    if (button) button.disabled = true;
    root.setAttribute('aria-busy', 'true');
    try {
      var response = await fetch(form.getAttribute('action') || window.location.href, {
        method: (form.getAttribute('method') || 'POST').toUpperCase(),
        body: new URLSearchParams(new FormData(form)),
        headers: { 'X-Component': '1', 'Content-Type': 'application/x-www-form-urlencoded' },
        credentials: 'same-origin',
      });
      var holder = document.createElement('template');
      holder.innerHTML = (await response.text()).trim();
      var next = holder.content.firstElementChild;
      if (!next) throw new Error('empty component');
      root.replaceWith(next);
      next.dispatchEvent(new CustomEvent('component:updated', { bubbles: true }));
    } catch (error) {
      form.submit(); // fall back to a normal page post
    }
  });
})();
