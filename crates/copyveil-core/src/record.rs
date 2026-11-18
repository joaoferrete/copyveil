//! The plaintext layout of a stored Item, before it is sealed.
//!
//! Each Item is two sealed values: a small `meta` record, rewritten when the
//! Item moves or is pinned, and a `content` record, written once. Both are
//! length-prefixed little-endian fields with a leading version byte, and both
//! decode strictly: any trailing byte, short field or unknown tag is refused.

use crate::crypto::CryptoError;
use crate::{Content, Kind};
use zeroize::Zeroizing;

const VERSION: u8 = 1;

const KIND_TEXT: u8 = 0;
const KIND_RICH_TEXT: u8 = 1;
const KIND_IMAGE: u8 = 2;

/// What the store knows about an Item besides its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Meta {
    /// Larger is newer. Only the store assigns it.
    pub(crate) order: u64,
    /// Milliseconds since the Unix epoch at the Item's last Capture.
    pub(crate) captured_ms: u64,
    /// Pin slot, counting from 1; 0 when not pinned.
    pub(crate) pin_slot: u64,
    /// Whether the user demoted the Item to Plain with an Override.
    pub(crate) overridden: bool,
    pub(crate) kind: Kind,
}

/// The content of an Item and its plain-text fallback.
pub(crate) struct Body {
    pub(crate) content: Content,
    pub(crate) plain_fallback: Option<Content>,
}

impl Meta {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = vec![VERSION];
        out.extend_from_slice(&self.order.to_le_bytes());
        out.extend_from_slice(&self.captured_ms.to_le_bytes());
        out.extend_from_slice(&self.pin_slot.to_le_bytes());
        out.push(u8::from(self.overridden));
        match &self.kind {
            Kind::Text => out.push(KIND_TEXT),
            Kind::RichText => out.push(KIND_RICH_TEXT),
            Kind::Image { mime } => {
                out.push(KIND_IMAGE);
                put_bytes(&mut out, mime.as_bytes());
            }
        }
        out
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, CryptoError> {
        let mut reader = Reader(bytes);
        reader.version()?;
        let order = reader.u64()?;
        let captured_ms = reader.u64()?;
        let pin_slot = reader.u64()?;
        let overridden = reader.flag()?;
        let kind = match reader.u8()? {
            KIND_TEXT => Kind::Text,
            KIND_RICH_TEXT => Kind::RichText,
            KIND_IMAGE => {
                let mime = String::from_utf8(reader.bytes()?.to_vec())
                    .map_err(|_| CryptoError::Unreadable)?;
                Kind::Image { mime }
            }
            _ => return Err(CryptoError::Unreadable),
        };
        reader.finish()?;
        Ok(Self {
            order,
            captured_ms,
            pin_slot,
            overridden,
            kind,
        })
    }
}

impl Body {
    /// The encoded body, zeroed when dropped since it is the content in clear.
    pub(crate) fn encode(
        content: &Content,
        plain_fallback: Option<&Content>,
    ) -> Zeroizing<Vec<u8>> {
        let mut out = Zeroizing::new(vec![VERSION]);
        out.push(u8::from(plain_fallback.is_some()));
        put_bytes(&mut out, content.expose());
        if let Some(fallback) = plain_fallback {
            put_bytes(&mut out, fallback.expose());
        }
        out
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, CryptoError> {
        let mut reader = Reader(bytes);
        reader.version()?;
        let has_fallback = reader.flag()?;
        let content = Content::new(reader.bytes()?.to_vec());
        let plain_fallback = if has_fallback {
            Some(Content::new(reader.bytes()?.to_vec()))
        } else {
            None
        };
        reader.finish()?;
        Ok(Self {
            content,
            plain_fallback,
        })
    }
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], CryptoError> {
        let (taken, rest) = self
            .0
            .split_at_checked(len)
            .ok_or(CryptoError::Unreadable)?;
        self.0 = rest;
        Ok(taken)
    }

    fn u8(&mut self) -> Result<u8, CryptoError> {
        Ok(self.take(1)?[0])
    }

    fn u64(&mut self) -> Result<u64, CryptoError> {
        let mut buf = [0; 8];
        buf.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(buf))
    }

    fn flag(&mut self) -> Result<bool, CryptoError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(CryptoError::Unreadable),
        }
    }

    fn version(&mut self) -> Result<(), CryptoError> {
        if self.u8()? == VERSION {
            Ok(())
        } else {
            Err(CryptoError::Unreadable)
        }
    }

    fn bytes(&mut self) -> Result<&'a [u8], CryptoError> {
        let len = usize::try_from(self.u64()?).map_err(|_| CryptoError::Unreadable)?;
        self.take(len)
    }

    fn finish(self) -> Result<(), CryptoError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(CryptoError::Unreadable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Body, Meta};
    use crate::crypto::CryptoError;
    use crate::{Content, Kind};
    use proptest::prelude::*;

    fn meta(kind: Kind) -> Meta {
        Meta {
            order: 0x0102_0304_0506_0708,
            captured_ms: 1_763_400_000_000,
            pin_slot: 3,
            overridden: true,
            kind,
        }
    }

    fn kinds() -> impl Strategy<Value = Kind> {
        prop_oneof![
            Just(Kind::Text),
            Just(Kind::RichText),
            "[a-z/+.-]{0,40}".prop_map(|mime| Kind::Image { mime }),
        ]
    }

    proptest! {
        #[test]
        fn meta_round_trips(
            order in any::<u64>(),
            captured_ms in any::<u64>(),
            pin_slot in any::<u64>(),
            overridden in any::<bool>(),
            kind in kinds(),
        ) {
            let meta = Meta { order, captured_ms, pin_slot, overridden, kind };
            prop_assert_eq!(Meta::decode(&meta.encode()).unwrap(), meta);
        }

        #[test]
        fn body_round_trips(
            content in proptest::collection::vec(any::<u8>(), 0..512),
            fallback in proptest::option::of(proptest::collection::vec(any::<u8>(), 0..512)),
        ) {
            let fallback = fallback.map(Content::new);
            let encoded = Body::encode(&Content::new(content.clone()), fallback.as_ref());
            let body = Body::decode(&encoded).unwrap();
            prop_assert_eq!(body.content.expose(), content.as_slice());
            prop_assert_eq!(
                body.plain_fallback.as_ref().map(Content::expose),
                fallback.as_ref().map(Content::expose)
            );
        }

        #[test]
        fn decoding_arbitrary_bytes_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..128)) {
            let _ = Meta::decode(&bytes);
            let _ = Body::decode(&bytes);
        }
    }

    #[test]
    fn meta_layout_is_fixed() {
        let encoded = meta(Kind::Image {
            mime: "image/png".into(),
        })
        .encode();
        let mut expected = vec![1];
        expected.extend_from_slice(&0x0102_0304_0506_0708_u64.to_le_bytes());
        expected.extend_from_slice(&1_763_400_000_000_u64.to_le_bytes());
        expected.extend_from_slice(&3_u64.to_le_bytes());
        expected.push(1);
        expected.push(2);
        expected.extend_from_slice(&9_u64.to_le_bytes());
        expected.extend_from_slice(b"image/png");
        assert_eq!(encoded, expected);
        assert_eq!(meta(Kind::Text).encode().last(), Some(&0));
        assert_eq!(meta(Kind::RichText).encode().last(), Some(&1));
    }

    #[test]
    fn body_layout_is_fixed() {
        let encoded = Body::encode(&Content::from("ab"), Some(&Content::from("c")));
        let mut expected = vec![1, 1];
        expected.extend_from_slice(&2_u64.to_le_bytes());
        expected.extend_from_slice(b"ab");
        expected.extend_from_slice(&1_u64.to_le_bytes());
        expected.extend_from_slice(b"c");
        assert_eq!(encoded.as_slice(), expected.as_slice());
        assert_eq!(&Body::encode(&Content::from("ab"), None)[..2], &[1, 0]);
    }

    #[test]
    fn trailing_bytes_are_refused() {
        let mut encoded = meta(Kind::Text).encode();
        encoded.push(0);
        assert_eq!(Meta::decode(&encoded), Err(CryptoError::Unreadable));

        let mut body = Body::encode(&Content::from("a"), None).to_vec();
        body.push(0);
        assert!(Body::decode(&body).is_err());
    }

    #[test]
    fn unknown_versions_tags_and_flags_are_refused() {
        let good = meta(Kind::Text).encode();

        let mut version = good.clone();
        version[0] = 2;
        assert_eq!(Meta::decode(&version), Err(CryptoError::Unreadable));

        let mut flag = good.clone();
        flag[25] = 2;
        assert_eq!(Meta::decode(&flag), Err(CryptoError::Unreadable));

        let mut kind = good;
        kind[26] = 3;
        assert_eq!(Meta::decode(&kind), Err(CryptoError::Unreadable));

        let mut body = Body::encode(&Content::from("a"), None).to_vec();
        body[1] = 2;
        assert!(Body::decode(&body).is_err());
    }

    #[test]
    fn short_fields_are_refused() {
        let encoded = meta(Kind::Image {
            mime: "image/png".into(),
        })
        .encode();
        for len in 0..encoded.len() {
            assert_eq!(
                Meta::decode(&encoded[..len]),
                Err(CryptoError::Unreadable),
                "prefix of length {len}"
            );
        }
        let body = Body::encode(&Content::from("ab"), Some(&Content::from("c")));
        for len in 0..body.len() {
            assert!(
                Body::decode(&body[..len]).is_err(),
                "prefix of length {len}"
            );
        }
    }

    #[test]
    fn invalid_utf8_mime_is_refused() {
        let mut encoded = meta(Kind::Image { mime: "a".into() }).encode();
        let last = encoded.len() - 1;
        encoded[last] = 0xff;
        assert_eq!(Meta::decode(&encoded), Err(CryptoError::Unreadable));
    }

    #[test]
    fn huge_lengths_are_refused() {
        let mut body = vec![1, 0];
        body.extend_from_slice(&u64::MAX.to_le_bytes());
        assert!(Body::decode(&body).is_err());
    }
}
