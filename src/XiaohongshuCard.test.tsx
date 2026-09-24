// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {XiaohongshuCard} from "./XiaohongshuCard";
import {api} from "./bridge";
vi.mock("./bridge",()=>({desktop:true,api:{xiaohongshuStatus:vi.fn(),xiaohongshuQrStart:vi.fn(),xiaohongshuQrPoll:vi.fn(),xiaohongshuQrCancel:vi.fn(),xiaohongshuOpen:vi.fn(),xiaohongshuFinish:vi.fn(),xiaohongshuLogout:vi.fn()}}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
it("shows the QR inside the app without opening a website and updates the account",async()=>{
 vi.mocked(api.xiaohongshuStatus).mockResolvedValue({sessionPresent:false,name:null,avatar:null});
 vi.mocked(api.xiaohongshuQrStart).mockResolvedValue("attempt-1");vi.mocked(api.xiaohongshuQrCancel).mockResolvedValue();
 vi.mocked(api.xiaohongshuQrPoll).mockResolvedValueOnce({loggedIn:false,image:"data:image/png;base64,test",profile:null}).mockResolvedValue({loggedIn:true,image:null,profile:{sessionPresent:true,name:"示例账号",avatar:null}});
 const change=vi.fn();render(<XiaohongshuCard onConnectionChange={change}/>);
 await waitFor(()=>expect(change).toHaveBeenCalledWith(false));fireEvent.click(screen.getByRole("button",{name:"扫码登录"}));
 await screen.findByRole("img",{name:"小红书登录二维码"});expect(api.xiaohongshuOpen).not.toHaveBeenCalled();
 await waitFor(()=>expect(change).toHaveBeenCalledWith(true),{timeout:3500});expect(screen.getByText("示例账号")).toBeTruthy();expect(screen.queryByRole("dialog")).toBeNull();
});
it("refreshes a QR attempt and cancels it on closing",async()=>{
 vi.mocked(api.xiaohongshuStatus).mockResolvedValue({sessionPresent:false,name:null,avatar:null});
 vi.mocked(api.xiaohongshuQrStart).mockResolvedValueOnce("old").mockResolvedValueOnce("new");vi.mocked(api.xiaohongshuQrCancel).mockResolvedValue();
 vi.mocked(api.xiaohongshuQrPoll).mockResolvedValue({loggedIn:false,image:"data:image/png;base64,test",profile:null});
 render(<XiaohongshuCard/>);fireEvent.click(screen.getByRole("button",{name:"扫码登录"}));await screen.findByRole("img",{name:"小红书登录二维码"});
 fireEvent.click(screen.getByRole("button",{name:"刷新二维码"}));await waitFor(()=>expect(api.xiaohongshuQrCancel).toHaveBeenCalledWith("old"));
 await waitFor(()=>expect(api.xiaohongshuQrPoll).toHaveBeenCalledWith("new"));fireEvent.click(screen.getByRole("button",{name:"关闭"}));await waitFor(()=>expect(api.xiaohongshuQrCancel).toHaveBeenCalledWith("new"));
});
it("restores a connection and reports logout failure without losing the account",async()=>{
 vi.mocked(api.xiaohongshuStatus).mockResolvedValue({sessionPresent:true,name:"示例账号",avatar:null});vi.mocked(api.xiaohongshuLogout).mockRejectedValueOnce("请先暂停下载").mockResolvedValue();
 render(<XiaohongshuCard/>);fireEvent.click(await screen.findByRole("button",{name:"断开连接"}));
 await screen.findByText("请先暂停下载");expect(screen.getByText("示例账号")).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"断开连接"}));await screen.findByRole("button",{name:"扫码登录"});
});
