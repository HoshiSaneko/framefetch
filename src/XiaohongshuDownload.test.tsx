// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { XiaohongshuDownload } from "./XiaohongshuDownload";
import { api, type XiaohongshuPreview } from "./bridge";
vi.mock("./bridge",()=>({api:{xiaohongshuPreview:vi.fn(),enqueueXiaohongshu:vi.fn()}}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const preview:XiaohongshuPreview={url:"https://www.bilibili.com/video/BV1xx411c7mD?p=1",title:"示例视频",thumbnail:"https://i0.hdslb.com/cover.jpg",formats:[{id:"80+30280",label:"1080P 高清"},{id:"64+30280",label:"720P 高清"}],hasAudio:true,images:[]};
const props=()=>({url:preview.url,onAdded:vi.fn().mockResolvedValue(undefined),onClose:vi.fn(),onBusyChange:vi.fn()});
it("parses automatically and submits the selected quality",async()=>{
 vi.mocked(api.xiaohongshuPreview).mockResolvedValue(preview);vi.mocked(api.enqueueXiaohongshu).mockResolvedValue();const p=props();render(<XiaohongshuDownload {...p}/>);
 await screen.findByRole("img",{name:"示例视频的封面"});
 expect(screen.queryByRole("button",{name:"连接 / 重新登录 Xiaohongshu"})).toBeNull();
 expect(screen.queryByText("仅显示当前账号可下载的格式，视频与声音会自动合并。")).toBeNull();
 fireEvent.click(screen.getByRole("combobox",{name:"视频清晰度"}));
 fireEvent.click(screen.getByRole("option",{name:"720P 高清"}));
 fireEvent.click(screen.getByRole("button",{name:"加入下载"}));
 await waitFor(()=>expect(api.enqueueXiaohongshu).toHaveBeenCalledWith(preview.url,{kind:"video",format:"64+30280"},[]));
 await waitFor(()=>expect(p.onClose).toHaveBeenCalledOnce());
});
it.each([['下载封面','cover'],['仅下载音频','audio']])("submits %s without a stale video format",async(label,kind)=>{
 vi.mocked(api.xiaohongshuPreview).mockResolvedValue(preview);vi.mocked(api.enqueueXiaohongshu).mockResolvedValue();render(<XiaohongshuDownload {...props()}/>);
 fireEvent.click(await screen.findByRole("button",{name:label}));expect(screen.queryByRole("combobox")).toBeNull();fireEvent.click(screen.getByRole("button",{name:"加入下载"}));
 await waitFor(()=>expect(api.enqueueXiaohongshu).toHaveBeenCalledWith(preview.url,{kind,format:null},[]));
});
it("ignores stale parses and prevents downloading while a new link resolves",async()=>{
 let finish!:(v:XiaohongshuPreview)=>void;vi.mocked(api.xiaohongshuPreview).mockReturnValueOnce(new Promise(resolve=>finish=resolve)).mockResolvedValueOnce({...preview,title:"新视频",url:preview.url+"&new=1"});
 const p=props();const view=render(<XiaohongshuDownload {...p}/>);await waitFor(()=>expect(api.xiaohongshuPreview).toHaveBeenCalledTimes(1));
 view.rerender(<XiaohongshuDownload {...p} url={preview.url+"&new=1"}/>);expect((screen.getByRole("button",{name:"加入下载"}) as HTMLButtonElement).disabled).toBe(true);
 await screen.findByText("新视频");finish(preview);await waitFor(()=>expect(screen.queryByText("示例视频")).toBeNull());
});
it("shows parse failure and recovers through retry",async()=>{
 vi.mocked(api.xiaohongshuPreview).mockRejectedValueOnce("登录已失效").mockResolvedValueOnce(preview);render(<XiaohongshuDownload {...props()}/>);
 await screen.findByRole("alert");expect((screen.getByRole("button",{name:"加入下载"}) as HTMLButtonElement).disabled).toBe(true);fireEvent.click(screen.getByRole("button",{name:"重新解析"}));await screen.findByText("示例视频");
});

it("selects only checked gallery images and blocks an empty selection",async()=>{
 vi.mocked(api.xiaohongshuPreview).mockResolvedValue({...preview,formats:[],hasAudio:false,images:["https://sns-webpic-qc.xhscdn.com/a.jpg","https://sns-webpic-qc.xhscdn.com/b.jpg"]});
 vi.mocked(api.enqueueXiaohongshu).mockResolvedValue();render(<XiaohongshuDownload {...props()}/>);
 const one=await screen.findByRole("checkbox",{name:"下载图片 1"});
 expect(screen.queryByRole("button",{name:"下载视频"})).toBeNull();
 fireEvent.click(screen.getByRole("button",{name:"取消全选"}));
 expect((screen.getByRole("button",{name:"加入下载"}) as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(one);fireEvent.click(screen.getByRole("button",{name:"加入下载"}));
 await waitFor(()=>expect(api.enqueueXiaohongshu).toHaveBeenCalledWith(preview.url,{kind:"images",format:null},[0]));
});
it("does not offer a quality dropdown when only one format exists",async()=>{
 vi.mocked(api.xiaohongshuPreview).mockResolvedValue({...preview,formats:[preview.formats[0]]});render(<XiaohongshuDownload {...props()}/>);
 await screen.findByText("示例视频");expect(screen.queryByRole("combobox")).toBeNull();
});
