// @vitest-environment jsdom
import { cleanup } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { workSummary, groupDownloads, batchProgress } from "./DownloadGroup";
import { designFixture } from "./designFixture";
const base = designFixture().tasks[0];
const first = {...base,id:"first",platform:"douyin" as const,url:"https://v.douyin.com/short/",fileName:"123-1.jpg",title:"图集 · 1"};
const second = {...first,id:"second",url:"https://www.douyin.com/video/123?image=1",fileName:"123-2.jpg",title:"图集 · 2"};
afterEach(cleanup);
it("groups Xiaohongshu images by persisted work identity and supports legacy galleries", () => {
  const storage = {base:"C:/downloads/小红书",workId:"album",directory:"C:/downloads/小红书/album",date:"2026-09-24"};
  const a = {...base, id:"xhs-1",platform:"xiaohongshu" as const,url:"https://www.xiaohongshu.com/explore/abc?token=a",fileName:"图片_001.jpg",storage,xiaohongshu:{kind:"image",imageIndex:0}};
  const b = {...a,id:"xhs-2",fileName:"图片_002.jpg",xiaohongshu:{kind:"image",imageIndex:1}};
  expect(groupDownloads([b,a])[0].items.map(t=>t.id)).toEqual([a.id,b.id]);
  expect(groupDownloads([a,b,{...a,id:"other",storage:{...storage,workId:"other"}}])).toHaveLength(2);
  expect(groupDownloads([{...a,storage:undefined},{...b,storage:undefined,url:a.url.replace("token=a","token=b")}])).toHaveLength(1);
  expect(batchProgress([a,b])).toEqual({total:1,completed:0});
});
it("keeps a persisted batch as one task while counting works instead of gallery images",()=>{
  const batch={id:"batch-one",title:"博主 · 全部作品"};
  const items=[{...first,batch,status:"completed" as const},{...second,batch,status:"completed" as const},{...first,id:"third",fileName:"456.mp4",url:"https://www.douyin.com/video/456",batch,status:"queued" as const}];
  const restored=JSON.parse(JSON.stringify(items));
  expect(groupDownloads(restored)).toHaveLength(1);
  expect(workSummary(restored).title).toBe(batch.title);
  expect(workSummary(restored).status).toBe("queued");
  expect(batchProgress(restored)).toEqual({total:2,completed:1});
  expect(groupDownloads([...restored,{...first,id:"separate",batch:{id:"other",title:batch.title}}])).toHaveLength(2);
});
it("groups separated gallery files by work identity, including legacy short links",()=>{
  const other={...first,id:"other",fileName:"456-1.jpg",url:"https://www.douyin.com/note/456"};
  const groups=groupDownloads([second,other,first,{...base,id:"telegram",title:first.title}]);
  expect(groups.map(g=>g.items.map(t=>t.id))).toEqual([["first","second"],["other"],["telegram"]]);
});
it("represents a gallery as one work with total size and an honest mixed status",()=>{
  const summary=workSummary([{...first,status:"completed",totalBytes:10,downloadedBytes:10},{...second,status:"failed",totalBytes:20,downloadedBytes:5}]);
  expect(summary.title).toBe("图集");
  expect(summary.totalBytes).toBe(30);
  expect(summary.downloadedBytes).toBe(15);
  expect(summary.status).toBe("failed");
});
