((request) => {
  if (location.origin !== 'https://www.douyin.com') return null;
  if (window.__ffDouyinRequest?.id === request.id) return window.__ffDouyinRequest.result || null;
  const state = window.__ffDouyinRequest = {id: request.id, result: null};
  const send = (path, params, body) => new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open(body ? 'POST' : 'GET', path + '?' + new URLSearchParams({device_platform: 'webapp', aid: '6383', ...params}));
    xhr.withCredentials = true; xhr.timeout = 15000;
    if (body) xhr.setRequestHeader('Content-Type', 'application/x-www-form-urlencoded');
    xhr.onload = () => {try {if (xhr.status !== 200) throw new Error(); resolve(JSON.parse(xhr.responseText));} catch {reject(new Error('抖音接口返回异常，请打开官网检查登录状态'));}};
    xhr.onerror = xhr.ontimeout = () => reject(new Error('抖音请求失败或超时，请稍后重试'));
    xhr.send(body ? new URLSearchParams(body).toString() : null);
  });
  (async () => {
    let result;
    const page = {count: '20', cursor: request.cursor || '0'};
    const collectionPage = {...page, count:'10', version_code:'170400', version_name:'17.4.0'};
    if (request.kind === 'detail') result = await send('/aweme/v1/web/aweme/detail/', {aweme_id: request.itemId});
    else if (request.kind === 'favorites') result = await send('/aweme/v1/web/aweme/listcollection/', {version_code:'170400',version_name:'17.4.0',publish_video_strategy_type:'2'}, page);
    else if (request.kind === 'folders') result = await send('/aweme/v1/web/collects/list/', collectionPage);
    else if (request.kind === 'folder') result = await send('/aweme/v1/web/collects/video/list/', {...collectionPage, collects_id: request.itemId});
    else if (request.kind === 'author') result = await send('/aweme/v1/web/aweme/post/', {sec_user_id:request.itemId, max_cursor:page.cursor, count:'18', locate_query:'false', show_live_replay_strategy:'1', need_time_list:'1', time_list_query:'0', whale_cut_token:'', cut_version:'1', publish_video_strategy_type:'2', from_user_page:'1'});
    else if (request.kind === 'likes') {
      const self = await send('/aweme/v1/web/user/profile/self/', {});
      if (self.status_code !== 0 || !self.user?.sec_uid) throw new Error('登录已失效，请重新扫码');
      result = await send('/aweme/v1/web/aweme/favorite/', {sec_user_id: self.user.sec_uid, count: '20', max_cursor: page.cursor});
    } else throw new Error('未知列表类型');
    if (result.status_code !== 0) throw new Error('抖音未允许此次请求，请在官网完成验证或重新登录');
    state.result = {data: result};
  })().catch(error => {state.result = {error: error.message};});
  return null;
})(__REQUEST__)
