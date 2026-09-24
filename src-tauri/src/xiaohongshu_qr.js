(() => {
  if (location.origin !== 'https://www.xiaohongshu.com') return null;
  const visible = el => { const r = el.getBoundingClientRect(); return r.width >= 100 && r.height >= 100; };
  // Official login page uses .qrcode-img with a PNG data URL.
  const qr = [...document.querySelectorAll('img.qrcode-img, .qrcode img, [class*="qr-code"] img')].find(visible);
  if (!qr || !qr.complete || !qr.naturalWidth) return null;
  if (qr.src.startsWith('data:image/png;base64,')) return qr.src;
  try {
    const canvas = document.createElement('canvas');
    canvas.width = qr.naturalWidth; canvas.height = qr.naturalHeight;
    canvas.getContext('2d').drawImage(qr, 0, 0);
    return canvas.toDataURL('image/png');
  } catch { return null; }
})()
