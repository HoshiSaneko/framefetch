// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { BilibiliDownload } from "./BilibiliDownload";
import { api, type BilibiliPreview } from "./bridge";
vi.mock("./bridge",()=>({api:{bilibiliPreview:vi.fn(),enqueueBilibili:vi.fn()}}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const preview:BilibiliPreview={url:"https://www.bilibili.com/video/BV1xx411c7mD?p=1",title:"示例视频",thumbnail:"https://i0.hdslb.com/cover.jpg",formats:[{id:"80+30280",label:"1080P 高清"},{id:"64+30280",label:"720P 高清"}],hasAudio:true};
const props=()=>({url:preview.url,onAdded:vi.fn().mockResolvedValue(undefined),onClose:vi.fn(),onBusyChange:vi.fn()});
it("shows video topics independently of its title", async()=>{
 vi.mocked(api.bilibiliPreview).mockResolvedValue({...preview,topics:["游戏","赛事"]});
 render(<BilibiliDownload {...props()}/>);
 fireEvent.click(await screen.findByRole("button",{name:"查看全部 2 个话题"}));
 expect(screen.getByRole("dialog",{name:"全部话题 · 2"})).toBeTruthy();
 expect(api.enqueueBilibili).not.toHaveBeenCalled();
});
it("parses automatically and submits the selected quality",async()=>{
 vi.mocked(api.bilibiliPreview).mockResolvedValue(preview);vi.mocked(api.enqueueBilibili).mockResolvedValue();const p=props();render(<BilibiliDownload {...p}/>);
 await screen.findByRole("img",{name:"示例视频的封面"});
 expect(screen.queryByRole("button",{name:"连接 / 重新登录 Bilibili"})).toBeNull();
 expect(screen.queryByText("仅显示当前账号可下载的格式，视频与声音会自动合并。")).toBeNull();
 fireEvent.click(screen.getByRole("combobox",{name:"视频清晰度"}));
 fireEvent.click(screen.getByRole("option",{name:"720P 高清"}));
 fireEvent.click(screen.getByRole("button",{name:"加入下载"}));
 await waitFor(()=>expect(api.enqueueBilibili).toHaveBeenCalledWith(preview.url,{kind:"video",format:"64+30280"},false));
 await waitFor(()=>expect(p.onClose).toHaveBeenCalledOnce());
});
it.each([['下载封面','cover'],['仅下载音频','audio']])("submits %s without a stale video format",async(label,kind)=>{
 vi.mocked(api.bilibiliPreview).mockResolvedValue(preview);vi.mocked(api.enqueueBilibili).mockResolvedValue();render(<BilibiliDownload {...props()}/>);
 fireEvent.click(await screen.findByRole("button",{name:label}));if(kind==="cover")fireEvent.click(screen.getByRole("button",{name:"下载视频"}));expect(screen.queryByRole("combobox")).toBeNull();fireEvent.click(screen.getByRole("button",{name:"加入下载"}));
 await waitFor(()=>expect(api.enqueueBilibili).toHaveBeenCalledWith(preview.url,{kind,format:null},false));
});
it("ignores stale parses and prevents downloading while a new link resolves",async()=>{
 let finish!:(v:BilibiliPreview)=>void;vi.mocked(api.bilibiliPreview).mockReturnValueOnce(new Promise(resolve=>finish=resolve)).mockResolvedValueOnce({...preview,title:"新视频",url:preview.url+"&new=1"});
 const p=props();const view=render(<BilibiliDownload {...p}/>);await waitFor(()=>expect(api.bilibiliPreview).toHaveBeenCalledTimes(1));
 view.rerender(<BilibiliDownload {...p} url={preview.url+"&new=1"}/>);expect((screen.getByRole("button",{name:"加入下载"}) as HTMLButtonElement).disabled).toBe(true);
 await screen.findByText("新视频");finish(preview);await waitFor(()=>expect(screen.queryByText("示例视频")).toBeNull());
});
it("shows parse failure and recovers through retry",async()=>{
 vi.mocked(api.bilibiliPreview).mockRejectedValueOnce("登录已失效").mockResolvedValueOnce(preview);render(<BilibiliDownload {...props()}/>);
 await screen.findByRole("alert");expect((screen.getByRole("button",{name:"加入下载"}) as HTMLButtonElement).disabled).toBe(true);fireEvent.click(screen.getByRole("button",{name:"重新解析"}));await screen.findByText("示例视频");
});

it("queues video and cover together and allows deselecting the cover",async()=>{
 vi.mocked(api.bilibiliPreview).mockResolvedValue(preview);vi.mocked(api.enqueueBilibili).mockResolvedValue();render(<BilibiliDownload {...props()}/>);
 const cover=await screen.findByRole("button",{name:"下载封面"});
 fireEvent.click(cover);
 expect(cover.getAttribute("aria-pressed")).toBe("true");
 expect(screen.getByRole("button",{name:"下载视频"}).getAttribute("aria-pressed")).toBe("true");
 fireEvent.click(cover);expect(cover.getAttribute("aria-pressed")).toBe("false");
 fireEvent.click(cover);fireEvent.click(screen.getByRole("button",{name:"加入下载"}));
 await waitFor(()=>expect(api.enqueueBilibili).toHaveBeenCalledWith(preview.url,{kind:"video",format:"80+30280"},true));
});
