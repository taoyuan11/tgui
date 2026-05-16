use crate::media::{ContentFit, MediaBytes, MediaSource};

/// 背景图像定义。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BackgroundImage {
    pub source: MediaSource,
    pub fit: ContentFit,
}

impl BackgroundImage {
    /// 通过媒体源创建背景图像。
    ///
    /// # 参数
    /// - `source`：背景图像来源。
    ///
    /// # 返回值
    /// 返回新的背景图像，默认 `fit` 为 `ContentFit::Cover`。
    pub fn new(source: impl Into<MediaSource>) -> Self {
        Self {
            source: source.into(),
            fit: ContentFit::Cover,
        }
    }

    /// 通过本地路径创建背景图像。
    pub fn from_path(path: impl Into<std::path::PathBuf>) -> Self {
        Self::new(MediaSource::Path(path.into()))
    }

    /// 通过网络地址创建背景图像。
    pub fn from_url(url: impl Into<String>) -> Self {
        Self::new(MediaSource::Url(url.into()))
    }

    /// 通过内存字节创建背景图像。
    pub fn from_bytes(bytes: impl Into<MediaBytes>) -> Self {
        Self::new(MediaSource::Bytes(bytes.into()))
    }

    /// 设置背景图像缩放方式。
    pub fn fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self
    }
}
