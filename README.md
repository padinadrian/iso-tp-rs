# ISO-TP / ISO-15765 in Rust

An implementation of the ISO-TP transport-layer protocol.

This protocol is frequently used as a transport protocol on top of CAN. Normally
CAN is limited to 8 bytes per packet. ISO-TP is a protocol that allows sending
longer messages by breaking them up into multiple packets.

The original version of ISO-TP (ISO-15765-1) can send messages up to 4095 bytes
long.

The second iteration (ISO-15765-2) can send messages up to 2<sup>32</sup> - 1.
However, this version is not yet supported by this library.

## Examples

Example of decoding a single-frame 7-byte message:

```rust
use iso_tp::Decoder;

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
```

## Supported Features

Currently supported:
* Decoding transfers (both single- and multi-frame transfers)
* ISO-15765-1 (up to 4095 bytes per transfer)
* CAN-2 (8 bytes per frame)

TODO:
* Encoding transfers
* ISO-15765-2 (up to 2^32 - 1 bytes per transfer)
* CAN-FD (64 bytes per frame)
