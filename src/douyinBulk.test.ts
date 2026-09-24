// @vitest-environment jsdom
import {afterEach, expect, it, vi} from "vitest";
import {api} from "./bridge";
import {downloadAllWorks} from "./douyinBulk";
import type {DownloadTask} from "./types";
afterEach(()=>vi.restoreAllMocks());
const item = (id:string) => ({id,title:`作品${id}`,author:"博主",cover:null,url:`https://www.douyin.com/video/${id}`,images:0});
function snapshot(ids:string[] = []) {
  vi.spyOn(api,"snapshot").mockResolvedValue({tasks:ids.map(id=>({url:item(id).url+"?image=1",status:"completed"} as DownloadTask)),settings:{downloadDir:"",concurrency:2,apiId:"",apiHash:"",proxyUrl:""},account:{connected:false,name:"",username:""}});
}
it("downloads every folder page, deduplicating overlapping pages and existing galleries",async()=>{
  snapshot(["1"]);
  const library=vi.spyOn(api,"douyinLibrary").mockResolvedValueOnce({items:[item("1"),item("2")],cursor:"10",hasMore:true}).mockResolvedValueOnce({items:[item("2"),item("3")],cursor:"20",hasMore:false});
  const enqueue=vi.spyOn(api,"enqueue").mockResolvedValue();
  expect(await downloadAllWorks("folder","789",new AbortController().signal,()=>{})).toEqual({added:2,skipped:1,stopped:false});
  expect(library.mock.calls).toEqual([["folder","0","789"],["folder","10","789"]]);
  expect(enqueue.mock.calls.map(call=>call[0])).toEqual([item("2").url,item("3").url]); expect(enqueue.mock.calls[0][2]).toEqual(enqueue.mock.calls[1][2]);
});
it("reuses resolved author IDs on subsequent pages",async()=>{
  snapshot();
  const library=vi.spyOn(api,"douyinLibrary").mockResolvedValueOnce({items:[item("1")],cursor:"100",hasMore:true,sourceId:"MS4w-example"}).mockResolvedValueOnce({items:[item("2")],cursor:"0",hasMore:false});
  vi.spyOn(api,"enqueue").mockResolvedValue();
  await downloadAllWorks("author","https://v.douyin.com/example/",new AbortController().signal,()=>{});
  expect(library).toHaveBeenLastCalledWith("author","100","MS4w-example");
});
it("reports pagination failure instead of claiming all works were downloaded",async()=>{
  snapshot();vi.spyOn(api,"douyinLibrary").mockResolvedValue({items:[item("1")],cursor:"0",hasMore:true});
  const enqueue=vi.spyOn(api,"enqueue").mockResolvedValue();
  await expect(downloadAllWorks("folder","789",new AbortController().signal,()=>{})).rejects.toThrow("分页未继续");
  expect(enqueue).toHaveBeenCalledTimes(1);
});
it("stops adding items and requesting pages after cancellation",async()=>{
  snapshot();const controller=new AbortController();
  const library=vi.spyOn(api,"douyinLibrary").mockResolvedValue({items:[item("1"),item("2")],cursor:"10",hasMore:true});
  const enqueue=vi.spyOn(api,"enqueue").mockImplementation(async()=>{controller.abort();});
  expect(await downloadAllWorks("folder","789",controller.signal,()=>{})).toEqual({added:1,skipped:0,stopped:true});
  expect(enqueue).toHaveBeenCalledTimes(1);expect(library).toHaveBeenCalledTimes(1);
});
it("stops on an enqueue failure and preserves already added progress",async()=>{
  snapshot();vi.spyOn(api,"douyinLibrary").mockResolvedValue({items:[item("1"),item("2"),item("3")],cursor:"10",hasMore:true});
  const enqueue=vi.spyOn(api,"enqueue").mockResolvedValueOnce().mockRejectedValue("磁盘不可用");const progress=vi.fn();
  await expect(downloadAllWorks("folder","789",new AbortController().signal,progress)).rejects.toThrow("作品2：磁盘不可用");
  expect(progress).toHaveBeenLastCalledWith(1,0,1);expect(enqueue).toHaveBeenCalledTimes(2);
});
