use bytes::Buf;
use prost::Message;
use prost_reflect::{DynamicMessage, MessageDescriptor};
use tonic::codec::Codec;

#[derive(Clone)]
pub struct DynamicMessageCodec {
    // Descriptor for the message type we are sending
    pub encode_desc: MessageDescriptor,
    // Descriptor for the message type we expect to receive
    pub decode_desc: MessageDescriptor,
}

impl Codec for DynamicMessageCodec {
    type Encode = DynamicMessage;
    type Decode = DynamicMessage;
    type Encoder = DynamicMessageEncoder;
    type Decoder = DynamicMessageDecoder;

    fn encoder(&mut self) -> Self::Encoder {
        DynamicMessageEncoder(self.encode_desc.clone())
    }

    fn decoder(&mut self) -> Self::Decoder {
        DynamicMessageDecoder(self.decode_desc.clone())
    }
}

pub struct DynamicMessageEncoder(MessageDescriptor);

impl tonic::codec::Encoder for DynamicMessageEncoder {
    type Item = DynamicMessage;
    type Error = tonic::Status;

    fn encode(
        &mut self,
        item: Self::Item,
        dst: &mut tonic::codec::EncodeBuf<'_>,
    ) -> Result<(), Self::Error> {
        item.encode(dst)
            .map_err(|e| tonic::Status::internal(format!("Encoding failed: {}", e)))
    }
}

pub struct DynamicMessageDecoder(MessageDescriptor);

impl tonic::codec::Decoder for DynamicMessageDecoder {
    type Item = DynamicMessage;
    type Error = tonic::Status;

    fn decode(
        &mut self,
        src: &mut tonic::codec::DecodeBuf<'_>,
    ) -> Result<Option<Self::Item>, Self::Error> {
        if !src.has_remaining() {
            return Ok(None);
        }
        let bytes = src.copy_to_bytes(src.remaining());
        let msg = DynamicMessage::decode(self.0.clone(), bytes)
            .map_err(|e| tonic::Status::internal(format!("Decoding failed: {}", e)))?;
        Ok(Some(msg))
    }
}