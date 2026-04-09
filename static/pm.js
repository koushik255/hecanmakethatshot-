async function loadPageMarks() {
  const res = await fetch('/api/pagemarks');
  if (!res.ok) {
    throw new Error(`Failed to fetch pagemarks: ${res.status}`);
  }

  const marks = await res.json();
  const list = document.getElementById('list');

  if (!Array.isArray(marks) || marks.length === 0) {
    list.innerHTML = '<div>No pagemarks yet.</div>';
    return;
  }

  list.innerHTML = marks
    .map((m, i) => {
      const rightSrc = `/api/image-by-path?path=${encodeURIComponent(m.pathright)}`;
      const leftImg = m.pathleft
        ? `<img src="/api/image-by-path?path=${encodeURIComponent(m.pathleft)}" style="max-width:360px;height:auto;border:1px solid #333;" />`
        : '';

      return `
        <div style="border:1px solid #333;padding:10px;border-radius:6px;background:#1b1b1b;">
          <div><strong>#${i + 1}</strong> volume: ${m.volume}</div>
          <div style="display:flex;gap:1px;flex-wrap:wrap;margin-top:8px;">
            ${leftImg}
            <img src="${rightSrc}" style="max-width:360px;height:auto;border:1px solid #333;" />
          </div>
        </div>
      `;
    })
    .join('');
}

loadPageMarks().catch(err => {
  document.body.innerHTML = `<pre>${err}</pre>`;
  console.error(err);
});
