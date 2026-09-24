(() => {
  if (location.protocol !== 'https:' || !(location.hostname === 'bilibili.com' || location.hostname.endsWith('.bilibili.com'))) return null;
  const visible = el => { const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0; };
  // Read only elements identified as login QR codes; never arbitrary page images.
  const nativeCanvas = [...document.querySelectorAll('[class*="qrcode"] canvas, [class*="qr-code"] canvas')].find(visible);
  if (nativeCanvas) { try { return nativeCanvas.toDataURL('image/png'); } catch {} }
  const qr = [...document.querySelectorAll('img[alt="Scan me!"], #animate_qrcode_container img, img[aria-label*="二维码"], img[alt*="二维码"], [class*="qrcode"] img, [class*="qr-code"] img')]
    .find(el => visible(el) && el.width >= 100 && el.height >= 100);
  if (qr) {
    if (qr.src.startsWith('data:image/png;base64,')) return qr.src;
    try {
      if (!qr.complete || !qr.naturalWidth) return null;
      const canvas = document.createElement('canvas');
      canvas.width = qr.naturalWidth; canvas.height = qr.naturalHeight;
      canvas.getContext('2d').drawImage(qr, 0, 0);
      return canvas.toDataURL('image/png');
    } catch { return null; }
  }
  // Let the website create and refresh the actual login challenge.
  const buttons = [...document.querySelectorAll('button, [role="button"]')].filter(visible);
  const target = buttons.find(el => ['扫码登录', '二维码登录'].includes(el.textContent.trim())) || buttons.find(el => el.textContent.trim() === '登录');
  if (target && (!window.__framefetchQrClick || Date.now() - window.__framefetchQrClick > 5000)) {
    window.__framefetchQrClick = Date.now(); target.click();
  }
  return null;
})()
