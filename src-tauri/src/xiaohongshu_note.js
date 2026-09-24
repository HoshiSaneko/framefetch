(() => {
  if (location.origin !== 'https://www.xiaohongshu.com') return null;
  if (location.pathname === '/login' || location.pathname === '/404') return {error: true};
  const id = __NOTE_ID__;
  if (!location.pathname.split('/').includes(id)) return null;
  const unwrap = v => v?._value ?? v?.value ?? v;
  const state = unwrap(window.__INITIAL_STATE__?.note);
  const note = unwrap(state?.noteDetailMap)?.[id]?.note;
  if (!note?.noteId && !note?.type) return null;
  return {note: {noteDetailMap: {[id]: {note: {
    noteId: note.noteId, type: note.type, title: note.title, desc: note.desc,
    imageList: note.imageList, video: note.video
  }}}}};
})()
