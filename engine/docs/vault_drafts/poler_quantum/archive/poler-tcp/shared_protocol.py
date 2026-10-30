"""poler-tcp/shared_protocol.py — Shared TCP Frame Protocol"""
import struct

MAGIC_HEADER = 0x504F4C52 # "POLR"

def pack_message(msg_type: int, payload: bytes) -> bytes:
    return struct.pack("!II", MAGIC_HEADER, msg_type) + payload

def unpack_header(data: bytes):
    magic, msg_type = struct.unpack("!II", data[:8])
    return magic == MAGIC_HEADER, msg_type
