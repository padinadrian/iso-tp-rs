//! Encoding and decoding ISO-TP messages.

/* ========== Imports ========== */
use num_enum::FromPrimitive;

/* ========== Exports ========== */
pub use crate::error::{Error, Result};

/* ========== Modules ========== */
pub mod error;

/* ========== Constants ========== */

/// Max number of data bytes per frame.
const MAX_DATA_BYTES_PER_FRAME: usize = 7;

/// Number of bytes in a single CAN frame.
const NUM_BYTES_PER_FRAME: usize = 8;

/// Maximum number of bytes that can be sent in a single transmission.
const MAX_BYTES_PER_TRANSFER: usize = 4095;

/* ========== Enums ========== */

/// Defines the Frame Type which determines what kind of data is contained in
/// the frame.
#[repr(u8)]
#[derive(Debug, Default, Copy, Clone, PartialEq, FromPrimitive)]
pub enum FrameType {
    /// The single frame transferred contains the complete payload.
    #[default]
    Single = 0,
    /// The first frame of a longer multi-frame message packet.
    First = 1,
    /// A frame containing subsequent data for a multi-frame packet.
    Consecutive = 2,
    /// The response from the receiver acknowledging the first-frame segment.
    FlowControl = 3,
}

/// Defines the Frame Type which determines what kind of data is contained in
/// the frame.
#[repr(u8)]
#[derive(Debug, Default, Copy, Clone, PartialEq, FromPrimitive)]
pub enum FlowControlStatus {
    /// Continue to send (transfer is allowed)
    Continue = 0,
    /// Wait to send (transfer should be delayed)
    Wait = 1,
    /// Overflow/abort (transfer is blocked)
    Overflow = 2,
    /// Unknown value
    #[default]
    Unknown = 3,
}

/* ========== Structs ========== */

/// Represents a single packet in an ISO-TP exchange.
#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub struct TransportData {
    index: u8,
    data_len: u8,
    data: [u8; MAX_DATA_BYTES_PER_FRAME],
}

/// Defines the Flow Control message, which is sent by the receiver in response
/// to the First Frame message.
#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub struct FlowControl {
    /// Indicates if the transfer is allowed.
    status: FlowControlStatus,
    /// Number of frames that can be safely sent before waiting for the next
    /// flow control frame from the receiver. A value of zero allows the
    /// remaining frames to be sent without flow control or delay.
    block_size: u8,
    /// Minimum requested time delay between frames.
    /// * Values 0-127: delay time in milliseconds
    /// * Values 241-249z: delay values increasing from 100-900 microseconds
    separation_time: u8,
}

/// Decode ISO-TP message.
#[derive(Debug, Clone)]
pub struct TransportDecoder<const N: usize> {
    /// Data packets collected so far.
    data: [u8; N],
    /// The expected number of bytes to receive.
    expected_length: u16,
    /// The number of bytes received so far.
    current_length: u16,
    /// Track what the next expected index is.
    next_index: u8,
}

impl<const N: usize> TransportDecoder<N> {
    pub const MAX_RECV_BYTES: usize = MAX_BYTES_PER_TRANSFER;
}

impl<const N: usize> TransportDecoder<N> {
    /// Create a new empty object.
    /// Must
    pub fn new() -> Self {
        Self {
            data: [0; N],
            expected_length: 0,
            current_length: 0,
            next_index: 0,
        }
    }

    /// Maximum size of transfer that this decoder can accept.
    pub const fn max_size(&self) -> usize {
        N
    }

    /// Update the decoder with a new frame. The input frame is expected to be
    /// 8 bytes long.
    /// * If the frame is complete and successfully decoded, returns Some(usize)
    ///   to indicate the data is ready, where the return value is the number of
    ///   bytes in the message.
    /// * If the frame is not ready, returns None.
    pub fn update(&mut self, frame: &[u8; NUM_BYTES_PER_FRAME]) -> Result<Option<usize>> {
        // Check frame type (upper four bits of first byte)
        let frame_type = FrameType::from(frame[0] >> 4);
        match frame_type {
            FrameType::Single => {
                // Data size is lower 4 bits of first byte
                let data_length = (frame[0] & 0xF) as usize;
                if data_length > MAX_DATA_BYTES_PER_FRAME {
                    return Err(Error::Overflow(
                        data_length as u16,
                        MAX_BYTES_PER_TRANSFER as u16,
                    ));
                }
                self.expected_length = data_length as u16;
                self.current_length = data_length as u16;
                self.data[0..data_length].copy_from_slice(&frame[1..(data_length + 1)]);
                return Ok(Some(data_length));
            }
            FrameType::First => {
                // Size is bytes 0.5 -> 2
                let mut expected_length = 0;
                expected_length += (frame[0] & 0xF) as u16;
                expected_length <<= 8;
                expected_length += frame[1] as u16;

                // Make sure internal buffer can handle this transfer.
                let max_size = self.max_size() as u16;
                if expected_length > max_size {
                    return Err(Error::BufferTooSmall(max_size, expected_length));
                }
                self.expected_length = expected_length;

                // The rest of this frame is the first chunk of data.
                let data_length = 6; // TODO: constant?
                self.data[0..data_length].copy_from_slice(&frame[2..]);
                self.current_length = data_length as u16;
                self.next_index = 1;
            }
            FrameType::Consecutive => {
                // Index increases by one every time, then rolls over after 15.
                let expected_index = self.next_index & 0xF;
                let actual_index = frame[0] & 0xF;
                if expected_index == actual_index {
                    self.next_index += 1;

                    // Copy data only up to expected length
                    // TODO: Is this check necessary? The only limit is the internal buffer size.
                    let data_remaining = (self.expected_length - self.current_length) as usize;
                    let data_length = std::cmp::min(MAX_DATA_BYTES_PER_FRAME, data_remaining);

                    let data_start = self.current_length as usize;
                    let data_end = data_start + data_length;
                    self.data[data_start..data_end].copy_from_slice(&frame[1..(data_length + 1)]);

                    self.current_length += data_length as u16;
                    if self.ready() {
                        return Ok(Some(self.current_length as usize));
                    } else {
                        return Ok(None);
                    }
                } else {
                    // TODO: Missed a frame; what do we do?
                    return Err(Error::MissedFrame(expected_index, actual_index));
                }
            }
            FrameType::FlowControl => {
                // TODO (?)
            }
        }

        Ok(None)
    }

    /// Returns true if the data is ready to view.
    pub fn ready(&self) -> bool {
        self.expected_length == self.current_length
    }

    /// Gets the completed data buffer, if ready.
    /// If not ready, returns None.
    pub fn data(&self) -> Option<&[u8]> {
        if self.ready() {
            let length = self.expected_length as usize;
            Some(&self.data[..length])
        } else {
            None
        }
    }
}

pub struct TransportEncoder {}

/* ========== Functions ========== */

#[cfg(test)]
mod tests {
    use super::*;

    /// Test decoding a Single Frame message of length 7.
    #[test]
    fn test_transport_decoder_single1() {
        let frame = [
            0x07, // Type = 0 (Single), Size = 7
            0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
        ];

        let mut decoder = TransportDecoder::<8>::new();
        let size = decoder.update(&frame).unwrap().unwrap();

        assert_eq!(size, 7);
        assert!(decoder.ready());
        assert_eq!(
            decoder.data().unwrap(),
            &[0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]
        );
    }

    /// Test decoding a Single Frame message of length 6.
    #[test]
    fn test_transport_decoder_single2() {
        let frame = [
            0x06, // Type = 0 (Single Frame), Size = 6
            0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
        ];

        let mut decoder = TransportDecoder::<8>::new();
        let size = decoder.update(&frame).unwrap().unwrap();

        assert_eq!(size, 6);
        assert!(decoder.ready());
        assert_eq!(
            decoder.data().unwrap(),
            &[0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE]
        );
    }

    /// Test decoding a Multiple Frame message of length 20.
    #[test]
    fn test_transport_decoder_multi1() {
        let frame1 = [
            0x10, // Type = 1 (First Frame)
            0x14, // Length = 20
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
        ];
        let frame2 = [
            0x21, // Type = 2 (Consecutive Frame), Index = 1
            0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
        ];
        let frame3 = [
            0x22, // Type = 2 (Consecutive Frame), Index = 2
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14,
        ];

        let mut decoder = TransportDecoder::<20>::new();

        let result = decoder.update(&frame1).unwrap();
        assert!(result.is_none());
        assert!(!decoder.ready());

        let result = decoder.update(&frame2).unwrap();
        assert!(result.is_none());
        assert!(!decoder.ready());

        let result = decoder.update(&frame3).unwrap();
        assert_eq!(result, Some(20));
        assert!(decoder.ready());
        assert_eq!(
            decoder.data().unwrap(),
            &[
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20
            ]
        );
    }
}
