import { expect, it } from "vitest";
import { splitXiaohongshuTopics, topicColor } from "./xiaohongshuTopics";

it("extracts native and plain topics, deduplicates them, and keeps the actual title", () => {
  expect(splitXiaohongshuTopics("到底谁说出门在外身份是自己给的 #搞笑[话题]# #搞笑段子[话题]# #旅行 #搞笑[话题]#"))
    .toEqual({title: "到底谁说出门在外身份是自己给的", topics: ["搞笑", "搞笑段子", "旅行"]});
  expect(splitXiaohongshuTopics("前文 #旅行[话题]# 后文").title).toBe("前文 后文");
  expect(splitXiaohongshuTopics("#旅行 #摄影").title).toBe("小红书作品");
  expect(splitXiaohongshuTopics("没有话题")).toEqual({title:"没有话题",topics:[]});
});
it("assigns stable colors from a varied palette", () => {
  const colors = ["搞笑", "搞笑段子", "旅行", "摄影"].map(topicColor);
  expect(colors).toEqual(["搞笑", "搞笑段子", "旅行", "摄影"].map(topicColor));
  expect(new Set(colors).size).toBeGreaterThan(1);
  expect(colors.every(color => color >= 0 && color < 6)).toBe(true);
});
it("shows separately saved topics even when the title has no hashtags", () => {
  expect(splitXiaohongshuTopics("终于找到了苹果手表睡眠监测不准的证据", ["苹果手表", "睡眠监测", "苹果手表"]))
    .toEqual({title: "终于找到了苹果手表睡眠监测不准的证据", topics: ["苹果手表", "睡眠监测"]});
  expect(splitXiaohongshuTopics("标题 #旅行[话题]#", ["旅行", "摄影"]).topics).toEqual(["旅行", "摄影"]);
});
