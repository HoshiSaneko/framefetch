import {readFileSync} from "node:fs";
import {runInNewContext} from "node:vm";
import {expect,it} from "vitest";
const script=readFileSync(new URL("../src-tauri/src/xiaohongshu_session.js",import.meta.url),"utf8");
const read=(user:unknown,origin="https://www.xiaohongshu.com")=>runInNewContext(script,{location:{origin},window:{__INITIAL_STATE__:{user}}});
it("distinguishes visitor cookies from an authenticated website account",()=>{
 expect(read({loggedIn:false,userInfo:{userId:"guest"}}).sessionPresent).toBe(false);
 expect(read({loggedIn:true,userInfo:{nickname:"示例",imageb:"https://sns-avatar-qc.xhscdn.com/a"}})).toMatchObject({sessionPresent:true,name:"示例"});
 expect(read({loggedIn:{_value:true},userInfo:{_value:{nickname:"示例"}}}).sessionPresent).toBe(true);
 expect(read({userInfo:{userId:"guest"}})).toBeNull();
 expect(read({loggedIn:true},"https://evil.test")).toBeNull();
});
