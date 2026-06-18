use crate::foundation::binding::Signal;
use crate::media::MediaSource;
use crate::ui::layout::Value;

/// 定义可转换为本地图片路径源的输入类型。
pub trait IntoImagePathSource {
    /// 将输入转换为图片路径源。
    ///
    /// # 返回值
    /// 返回静态或响应式的 `MediaSource::Path`。
    fn into_image_path_source(self) -> Value<MediaSource>;
}

/// 定义可转换为网络图片地址源的输入类型。
pub trait IntoImageUrlSource {
    /// 将输入转换为图片 URL 源。
    ///
    /// # 返回值
    /// 返回静态或响应式的 `MediaSource::Url`。
    fn into_image_url_source(self) -> Value<MediaSource>;
}

impl IntoImagePathSource for std::path::PathBuf {
    fn into_image_path_source(self) -> Value<MediaSource> {
        MediaSource::Path(self).into()
    }
}

impl IntoImagePathSource for &std::path::Path {
    fn into_image_path_source(self) -> Value<MediaSource> {
        MediaSource::Path(self.to_path_buf()).into()
    }
}

impl IntoImagePathSource for String {
    fn into_image_path_source(self) -> Value<MediaSource> {
        MediaSource::Path(self.into()).into()
    }
}

impl IntoImagePathSource for &str {
    fn into_image_path_source(self) -> Value<MediaSource> {
        MediaSource::Path(self.into()).into()
    }
}

impl IntoImagePathSource for Signal<std::path::PathBuf> {
    fn into_image_path_source(self) -> Value<MediaSource> {
        self.map(MediaSource::Path).into()
    }
}

impl IntoImagePathSource for Signal<String> {
    fn into_image_path_source(self) -> Value<MediaSource> {
        self.map(|path| MediaSource::Path(path.into())).into()
    }
}

impl IntoImagePathSource for Value<std::path::PathBuf> {
    fn into_image_path_source(self) -> Value<MediaSource> {
        match self {
            Value::Static(path) => MediaSource::Path(path).into(),
            Value::Signal(signal) => signal.map(MediaSource::Path).into(),
        }
    }
}

impl IntoImagePathSource for Value<String> {
    fn into_image_path_source(self) -> Value<MediaSource> {
        match self {
            Value::Static(path) => MediaSource::Path(path.into()).into(),
            Value::Signal(signal) => signal.map(|path| MediaSource::Path(path.into())).into(),
        }
    }
}

impl IntoImageUrlSource for String {
    fn into_image_url_source(self) -> Value<MediaSource> {
        MediaSource::Url(self).into()
    }
}

impl IntoImageUrlSource for &str {
    fn into_image_url_source(self) -> Value<MediaSource> {
        MediaSource::Url(self.into()).into()
    }
}

impl IntoImageUrlSource for Signal<String> {
    fn into_image_url_source(self) -> Value<MediaSource> {
        self.map(MediaSource::Url).into()
    }
}

impl IntoImageUrlSource for Value<String> {
    fn into_image_url_source(self) -> Value<MediaSource> {
        match self {
            Value::Static(url) => MediaSource::Url(url).into(),
            Value::Signal(signal) => signal.map(MediaSource::Url).into(),
        }
    }
}
