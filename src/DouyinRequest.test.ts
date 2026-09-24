import { expect, it } from "vitest";
import script from "../src-tauri/src/douyin_api.js?raw";

it.each([
  ["folders","/aweme/v1/web/collects/list/","cursor","10"],
  ["folder","/aweme/v1/web/collects/video/list/","cursor","10"],
  ["author","/aweme/v1/web/aweme/post/","max_cursor","18"],
])("sends the %s source and continuation fields", async (kind,path,cursorField,count) => {
  let request!: URL;
  class XHR {
    status=200; responseText=JSON.stringify({status_code:0,aweme_list:[],has_more:false});onload?:()=>void;
    open(_method:string,url:string) {request=new URL(url,"https://www.douyin.com");}
    setRequestHeader() {} send() {this.onload?.();}
  }
  new Function("window","location","XMLHttpRequest",script.replace("__REQUEST__",JSON.stringify({id:kind,kind,cursor:"123456",itemId:"source-id"})))({},{origin:"https://www.douyin.com"},XHR);
  await new Promise(resolve=>setTimeout(resolve,0));
  expect(request.pathname).toBe(path);
  expect(request.searchParams.get(cursorField)).toBe("123456");
  expect(request.searchParams.get("count")).toBe(count);
  if(kind==="author") expect(request.searchParams.get("sec_user_id")).toBe("source-id");
  else expect(request.searchParams.get("version_code")).toBe("170400");
  if(kind==="folder") expect(request.searchParams.get("collects_id")).toBe("source-id");
});

it("sends the next page cursor with the authenticated user's ID", async () => {
  const requests: URL[] = [];
  const state: {__ffDouyinRequest?: {result?: unknown}} = {};
  class XHR {
    status = 200; responseText = ""; onload?: () => void;
    url!: URL;
    open(_method: string, path: string) {this.url = new URL(path,"https://www.douyin.com"); requests.push(this.url);}
    setRequestHeader() {}
    send() {
      this.responseText = JSON.stringify(this.url.pathname.includes('profile/self') ? {status_code:0,user:{sec_uid:'test-user'}} : {status_code:0,aweme_list:[],max_cursor:40,has_more:1});
      this.onload?.();
    }
  }
  const run = new Function("window","location","performance","XMLHttpRequest",script.replace('__REQUEST__',JSON.stringify({id:'test',kind:'likes',cursor:'20'})));
  run(state,{origin:'https://www.douyin.com'},{getEntriesByType:()=>[{name:'https://www.douyin.com/aweme/v1/web/aweme/favorite/?channel=channel_pc_web&max_cursor=0&a_bogus=old&pc_client_type=1'}]},XHR);
  await new Promise(resolve=>setTimeout(resolve,0));
  const request=requests[1];
  expect(request.searchParams.get('max_cursor')).toBe('20');
  expect(request.searchParams.get('sec_user_id')).toBe('test-user');
  expect(state.__ffDouyinRequest?.result).toEqual({data:{status_code:0,aweme_list:[],max_cursor:40,has_more:1}});
});
