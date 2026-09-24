(() => {
  if (location.origin !== 'https://www.xiaohongshu.com') return null;
  const unwrap = value => value?._value ?? value?.value ?? value;
  const user = unwrap(window.__INITIAL_STATE__?.user);
  if (!user) return null;
  const info = unwrap(user.userInfo) || {};
  const loggedIn = unwrap(user.loggedIn ?? user.isLogin);
  // web_session also exists for visitors; never infer account login from cookies alone.
  if (typeof loggedIn !== 'boolean') return null;
  return {sessionPresent: loggedIn, name: loggedIn ? (info.nickname || info.nickName || '小红书账号') : null,
    avatar: loggedIn ? (info.imageb || info.images || info.avatar || null) : null};
})()
