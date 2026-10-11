"""
Gafana Python Production Binding (ctypes)
Compatible with Python 3.8+ on Windows, Linux, and macOS.
Allows embedding libgafana directly into PyQt, PySide, Tkinter, or headless server agents.
"""

import ctypes
import os
from ctypes import (
    c_int, c_uint32, c_char_p, c_void_p, c_bool, c_size_t, c_float,
    Structure, CFUNCTYPE, POINTER, c_uint16
)

# Search for compiled dynamic library
lib_path = os.environ.get("GAFANA_LIB_PATH")
if not lib_path:
    cur_dir = os.path.dirname(os.path.abspath(__file__))
    candidates = [
        os.path.join(cur_dir, "../../gafana.dll"),
        os.path.join(cur_dir, "../../libgafana.so"),
        os.path.join(cur_dir, "../../libgafana.dylib"),
    ]
    for c in candidates:
        if os.path.exists(c):
            lib_path = os.path.abspath(c)
            break

class GafanaPeerInfo(Structure):
    _fields_ = [
        ("peer_id", ctypes.c_char * 64),
        ("display_name", ctypes.c_char * 128),
        ("is_speaking", c_bool),
        ("has_video", c_bool),
        ("has_screen", c_bool),
        ("has_presentation", c_bool),
        ("has_granted_recording", c_bool),
        ("join_timestamp_sec", c_uint32),
        ("last_heartbeat_ms", c_uint32),
    ]

class GafanaConfig(Structure):
    _fields_ = [
        ("relay_host", ctypes.c_char * 256),
        ("relay_port", c_uint16),
        ("relay_mode", c_int),
        ("max_peers", c_uint32),
        ("video_bitrate_kbps", c_uint32),
        ("audio_bitrate_kbps", c_uint32),
        ("ballot_timeout_sec", c_uint32),
        ("enable_e2ee", c_bool),
        ("e2ee_passphrase", ctypes.c_char * 128),
    ]

class GafanaPresentationEvent(Structure):
    _fields_ = [
        ("type", c_int),
        ("current_slide", c_uint32),
        ("total_slides", c_uint32),
        ("pointer_x", c_float),
        ("pointer_y", c_float),
        ("color_hex", c_uint32),
    ]

OnPeerJoinedCB = CFUNCTYPE(None, c_void_p, POINTER(GafanaPeerInfo), c_void_p)
OnPeerLeftCB = CFUNCTYPE(None, c_void_p, c_char_p, c_char_p, c_void_p)
OnRecordingConsentRequestedCB = CFUNCTYPE(None, c_void_p, c_uint32, c_char_p, c_int, c_void_p)
OnRecordingVoteUpdateCB = CFUNCTYPE(None, c_void_p, c_uint32, c_uint32, c_uint32, c_void_p)
OnRecordingStateChangedCB = CFUNCTYPE(None, c_void_p, c_uint32, c_int, c_char_p, c_void_p)

class GafanaCallbacks(Structure):
    _fields_ = [
        ("on_peer_joined", OnPeerJoinedCB),
        ("on_peer_left", OnPeerLeftCB),
        ("on_track_published", c_void_p),
        ("on_track_unpublished", c_void_p),
        ("on_video_frame", c_void_p),
        ("on_audio_frame", c_void_p),
        ("on_recording_consent_requested", OnRecordingConsentRequestedCB),
        ("on_recording_vote_update", OnRecordingVoteUpdateCB),
        ("on_recording_state_changed", OnRecordingStateChangedCB),
        ("on_presentation_event", c_void_p),
        ("on_room_joined", c_void_p),
        ("on_connection_state", c_void_p),
        ("on_error", c_void_p),
    ]

class GafanaEngine:
    def __init__(self, dll_path=None):
        raw_target = dll_path or lib_path
        if not raw_target or not os.path.exists(raw_target):
            raise FileNotFoundError(f"Native Gafana library not found at: {raw_target}")

        target = os.path.abspath(raw_target)
        if hasattr(os, 'add_dll_directory'):
            try:
                os.add_dll_directory(os.path.dirname(target))
            except Exception:
                pass
        self.lib = ctypes.CDLL(target)
        self._setup_prototypes()
        self.engine_ptr = c_void_p()

    def _setup_prototypes(self):
        self.lib.gafana_get_version_string.restype = c_char_p
        self.lib.gafana_config_default.argtypes = [POINTER(GafanaConfig)]
        self.lib.gafana_engine_create.argtypes = [POINTER(GafanaConfig), POINTER(GafanaCallbacks), c_void_p, POINTER(c_void_p)]
        self.lib.gafana_engine_create.restype = c_int
        self.lib.gafana_engine_destroy.argtypes = [c_void_p]
        self.lib.gafana_room_join.argtypes = [c_void_p, c_char_p, c_char_p, c_char_p]
        self.lib.gafana_room_join.restype = c_int
        self.lib.gafana_room_leave.argtypes = [c_void_p]
        self.lib.gafana_room_leave.restype = c_int
        self.lib.gafana_engine_poll.argtypes = [c_void_p]
        self.lib.gafana_engine_poll.restype = c_int
        self.lib.gafana_get_peer_count.argtypes = [c_void_p]
        self.lib.gafana_get_peer_count.restype = c_uint32
        self.lib.gafana_recording_request_consent.argtypes = [c_void_p, c_int, POINTER(c_uint32)]
        self.lib.gafana_recording_request_consent.restype = c_int
        self.lib.gafana_recording_cast_vote.argtypes = [c_void_p, c_uint32, c_int]
        self.lib.gafana_recording_cast_vote.restype = c_int
        self.lib.gafana_recording_start_local.argtypes = [c_void_p, c_uint32, c_char_p]
        self.lib.gafana_recording_start_local.restype = c_int
        self.lib.gafana_recording_stop_local.argtypes = [c_void_p]
        self.lib.gafana_recording_stop_local.restype = c_int
        self.lib.gafana_recording_revoke_consent.argtypes = [c_void_p, c_uint32]
        self.lib.gafana_recording_revoke_consent.restype = c_int

    def version(self):
        return self.lib.gafana_get_version_string().decode("utf-8")
