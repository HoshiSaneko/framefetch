use url::Url;

#[derive(Debug, PartialEq)]
pub enum Destination {
    Public(String),
    Private(i64),
}
#[derive(Debug, PartialEq)]
pub struct MessageLink {
    pub destination: Destination,
    pub message_id: i32,
    pub canonical: String,
}

pub fn parse_link(input: &str) -> Result<MessageLink, String> {
    let url = Url::parse(input.trim()).map_err(|_| "请输入完整的 Telegram 消息链接")?;
    if !["http", "https"].contains(&url.scheme())
        || !["t.me", "telegram.me", "www.t.me"].contains(&url.host_str().unwrap_or(""))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err("仅支持 t.me 或 telegram.me 的消息链接".into());
    }
    let mut parts: Vec<_> = url
        .path_segments()
        .ok_or("消息链接无效")?
        .filter(|s| !s.is_empty())
        .collect();
    if parts.first() == Some(&"s") {
        parts.remove(0);
    }
    // Forum message links insert the topic ID before the actual message ID.
    // Messages are fetched by channel and message ID, independent of the topic.
    let topic_index = if parts.first() == Some(&"c") { 2 } else { 1 };
    if parts.len() == topic_index + 2 {
        parts[topic_index].parse::<i32>().ok().filter(|id| *id > 0)
            .ok_or("话题 ID 无效")?;
        parts.remove(topic_index);
    }
    let (destination, id, canonical) = if parts.first() == Some(&"c") && parts.len() == 3 {
        let channel = parts[1]
            .parse::<i64>()
            .ok()
            .filter(|x| *x > 0)
            .ok_or("频道 ID 无效")?;
        (
            Destination::Private(channel),
            parts[2],
            format!("https://t.me/c/{}/{}", channel, parts[2]),
        )
    } else if parts.len() == 2
        && parts[0].len() >= 3
        && parts[0]
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        (
            Destination::Public(parts[0].to_string()),
            parts[1],
            format!("https://t.me/{}/{}", parts[0], parts[1]),
        )
    } else {
        return Err("请复制具体消息链接，不支持邀请链接或频道首页".into());
    };
    let message_id = id
        .parse::<i32>()
        .ok()
        .filter(|x| *x > 0)
        .ok_or("消息 ID 无效")?;
    Ok(MessageLink {
        destination,
        message_id,
        canonical,
    })
}

pub fn safe_file_name(input: &str) -> String {
    let replaced: String = input
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(180)
        .collect();
    let trimmed = replaced.trim_matches(|c| c == '.' || c == ' ');
    if trimmed.is_empty() {
        "telegram-file.bin".into()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_links() {
        assert_eq!(
            parse_link("https://t.me/s/channel/12?single")
                .unwrap()
                .canonical,
            "https://t.me/channel/12"
        );
        assert_eq!(
            parse_link("https://t.me/c/456/12").unwrap().destination,
            Destination::Private(456)
        );
    }
    #[test]
    fn resolves_forum_messages_to_the_message_not_the_topic() {
        let link = parse_link("https://t.me/c/3942745692/3437/15942").unwrap();
        assert_eq!(link.destination, Destination::Private(3942745692));
        assert_eq!(link.message_id, 15942);
        assert_eq!(link.canonical, "https://t.me/c/3942745692/15942");
        assert_eq!(parse_link("https://t.me/channel/3437/15942").unwrap().canonical,
            "https://t.me/channel/15942");
    }
    #[test]
    fn rejects_invalid_links() {
        for link in [
            "https://t.me.evil.org/a/12",
            "https://t.me/+abc",
            "file:///channel/12",
            "https://t.me/channel/0",
            "https://t.me/c/1/2/3/4",
            "https://t.me/c/1/0/3",
            "https://t.me/channel/2147483648/12",
            "https://t.me/channel/2147483648",
        ] {
            assert!(parse_link(link).is_err(), "{link}");
        }
    }
    #[test]
    fn filenames_cannot_escape_destination() {
        assert_eq!(safe_file_name("../../secret.txt"), "_.._secret.txt");
        assert_eq!(safe_file_name("  ... "), "telegram-file.bin");
        assert!(!safe_file_name("a:b\\c/d\0").contains(['/', '\\', ':', '\0']));
    }
}
