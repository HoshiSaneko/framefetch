(() => {
  if (location.origin !== 'https://www.douyin.com') return null;
  if (!window.__framefetchProfilePending) {
    window.__framefetchProfilePending = true;
    fetch('/aweme/v1/web/user/profile/self/?device_platform=webapp&aid=6383', {credentials: 'include'})
      .then(r => { if (!r.ok) throw new Error(); return r.json(); })
      .then(data => {
        const user = data.status_code === 0 && data.user;
        window.__framefetchProfile = user && typeof user.nickname === 'string'
          ? {name: user.nickname.slice(0, 120), avatar: user.avatar_thumb?.url_list?.[0] || user.avatar_medium?.url_list?.[0] || null}
          : {error: true};
      }).catch(() => {window.__framefetchProfile = {error: true};});
  }
  return window.__framefetchProfile || null;
})()
