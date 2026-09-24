export function splitXiaohongshuTopics(value: string, savedTopics: string[] = [], fallbackTitle = "小红书作品") {
  const topics: string[] = [...new Set(savedTopics.map(topic => topic.trim()).filter(Boolean))];
  const title = value.replace(/#([^#\s]+?)\[话题\]#|#([^#\s，。！？、；：,!?;:]+)/gu, (_, marked: string, plain: string) => {
    const topic = (marked || plain).trim();
    if (topic && !topics.includes(topic)) topics.push(topic);
    return " ";
  }).replace(/\s+/g, " ").trim();
  return {title: title || fallbackTitle, topics};
}

export function topicColor(topic: string) {
  let hash = 0;
  for (const character of topic) hash = (Math.imul(hash, 31) + character.codePointAt(0)!) >>> 0;
  return hash % 6;
}
