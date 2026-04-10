function preloadImage(url) {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => resolve({ ok: true, url });
    img.onerror = () => resolve({ ok: false, url });
    img.src = url;
  });
}

function waitForImageLoad(img) {
  if (img.complete && img.naturalWidth > 0) return Promise.resolve();
  return new Promise((resolve, reject) => {
    img.onload = () => resolve();
    img.onerror = () => reject(new Error('failed to load image'));
  });
}

async function loadSpread() {
  const stamp = Date.now();
  const rightUrl = `/api/right?v=${stamp}`;
  const leftUrl = `/api/left?v=${stamp}`;

  const [right, left] = await Promise.all([
    preloadImage(rightUrl),
    preloadImage(leftUrl),
  ]);

  if (!right.ok) {
    throw new Error('Failed to load right page');
  }

  const spreadView = document.getElementById('spreadView');
  const soloView = document.getElementById('soloView');
  const leftPhoto = document.getElementById('leftPhoto');
  const rightPhoto = document.getElementById('rightPhoto');
  const soloPhoto = document.getElementById('soloPhoto');

  if (!left.ok) {
    spreadView.style.display = 'none';
    soloView.style.display = 'flex';

    leftPhoto.removeAttribute('src');
    rightPhoto.removeAttribute('src');
    soloPhoto.src = right.url;

    await waitForImageLoad(soloPhoto);
    const landscape = soloPhoto.naturalWidth > soloPhoto.naturalHeight;
    soloPhoto.style.width = landscape ? '1050px' : '700px';
  } else {
    soloView.style.display = 'none';
    spreadView.style.display = 'flex';

    soloPhoto.removeAttribute('src');
    rightPhoto.src = right.url;
    leftPhoto.src = left.url;
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
