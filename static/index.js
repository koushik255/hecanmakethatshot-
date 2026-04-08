function setImageUrl(id, objectUrl) {
  const img = document.getElementById(id);

  if (img.dataset.url) {
    URL.revokeObjectURL(img.dataset.url);
    img.dataset.url = '';
  }

  if (!objectUrl) {
    img.removeAttribute('src');
    return;
  }

  img.dataset.url = objectUrl;
  img.src = objectUrl;
}

async function fetchImage(url, allowNoContent = false) {
  const res = await fetch(url);

  if (allowNoContent && res.status === 204) {
    return { noContent: true };
  }

  if (!res.ok) {
    throw new Error(`Failed to fetch ${url}: ${res.status}`);
  }

  const bytes = await res.arrayBuffer();
  const blob = new Blob([bytes], { type: 'image/jpeg' });
  return { noContent: false, objectUrl: URL.createObjectURL(blob) };
}

function waitForImageLoad(img) {
  if (img.complete && img.naturalWidth > 0) return Promise.resolve();
  return new Promise((resolve, reject) => {
    img.onload = () => resolve();
    img.onerror = () => reject(new Error('failed to load image'));
  });
}

async function loadSpread() {
  const right = await fetchImage('/api/right');
  const left = await fetchImage('/api/left', true);

  const spreadView = document.getElementById('spreadView');
  const soloView = document.getElementById('soloView');

  if (left.noContent) {
    spreadView.style.display = 'none';
    soloView.style.display = 'flex';

    setImageUrl('leftPhoto', null);
    setImageUrl('rightPhoto', null);
    setImageUrl('soloPhoto', right.objectUrl);

    const solo = document.getElementById('soloPhoto');
    await waitForImageLoad(solo);
    const landscape = solo.naturalWidth > solo.naturalHeight;
    solo.style.width = landscape ? '1050px' : '700px';
  } else {
    soloView.style.display = 'none';
    spreadView.style.display = 'flex';

    setImageUrl('soloPhoto', null);
    setImageUrl('rightPhoto', right.objectUrl);
    setImageUrl('leftPhoto', left.objectUrl);
  }
}

async function nextPage() {
  const res = await fetch('/api/next');
  if (res.status === 200) {
    await loadSpread();
  } else if (res.status === 204) {
    console.log('Already at last spread');
  } else {
    console.error('Failed to go to next spread', res.status);
  }
}

async function prevPage() {
  const res = await fetch('/api/prev');
  if (res.status === 200) {
    await loadSpread();
  } else if (res.status === 204) {
    console.log('Already at first spread');
  } else {
    console.error('Failed to go to prev spread', res.status);
  }
}

async function addBookmark() {
  const res = await fetch('/api/bookmark', { method: 'POST' });
  if (res.status === 201) {
    console.log('bookmark saved');
  } else {
    console.error('Failed to save bookmark', res.status);
  }
}

async function addPageMark() {
  const res = await fetch('/api/pagemark', { method: 'POST' });
  if (res.status === 201) {
    console.log('pagemark saved');
  } else {
    console.error('Failed to save pagemark', res.status);
  }
}

window.addEventListener('keydown', async (e) => {
  if (e.code === 'Space' || e.code === 'ArrowRight') {
    e.preventDefault();
    await nextPage();
  } else if (e.code === 'ArrowLeft') {
    e.preventDefault();
    await prevPage();
  } else if (e.key === 'm' || e.key === 'M') {
    e.preventDefault();
    await addBookmark();
  } else if (e.key === 't' || e.key === 'T') {
    e.preventDefault();
    await addPageMark();
  }
});

loadSpread().catch(err => {
  document.body.innerHTML = `<pre>${err}</pre>`;
  console.error(err);
});
