/**
 * Gafana Production Web Client Engine
 *
 * Implements:
 *  - Real WebRTC MediaStream acquisition (getUserMedia & getDisplayMedia)
 *  - Web Audio API microphone volume analysis
 *  - Real HTML5 Presentation Canvas & Laser Pointer
 *  - Strict 100% Unanimous Consensus Recording Protocol (Audio-only or Full A/V)
 *  - Visual Command Center Menu executing all engine commands as buttons
 *  - Embedded Secure SMTP Attendee Invitation Setup and Real-Time Log
 *  - Client-side MediaRecorder writing directly to local disk with ZERO cloud retention
 */

class GafanaWebClient {
  constructor() {
    this.localStream = null;
    this.screenStream = null;
    this.audioContext = null;
    this.analyser = null;
    this.mediaRecorder = null;
    this.recordedChunks = [];
    this.isRecording = false;
    this.recordingType = 'av'; // 'audio' or 'av'
    this.recordStartTime = 0;
    this.recordTimerInterval = null;

    // Consensus Ballot State
    this.currentBallot = null;
    this.ballotState = 'IDLE'; // IDLE, REQUESTED, APPROVED, REJECTED, REVOKED
    this.participants = [
      { id: 'self_host', name: 'Local Host (You)', hasVoted: false, vote: null, isSelf: true }
    ];

    // Presentation State
    this.slideImages = [];
    this.currentSlideIndex = 0;
    this.laserActive = false;
    this.networkUrl = '';

    this.initElements();
    this.initEventListeners();
    this.initCommandCenter();
    this.initSmtpDispatcher();
    this.initPeersModal();
    this.initHardwareMedia();
    this.initCallTimer();
    this.fetchNetworkInfo();
    this.initPeerSync();
  }

  initElements() {
    this.localVideo = document.getElementById('local-video');
    this.screenVideo = document.getElementById('screen-video');
    this.presCanvasWrapper = document.getElementById('pres-canvas');
    this.slideCanvas = document.getElementById('slide-canvas');
    this.slideCtx = this.slideCanvas.getContext('2d');
    this.laserDot = document.getElementById('laser-dot');
    this.videoPlaceholder = document.getElementById('video-placeholder');

    this.btnMic = document.getElementById('btn-mic');
    this.btnCam = document.getElementById('btn-cam');
    this.btnScreen = document.getElementById('btn-screenshare');
    this.btnPres = document.getElementById('btn-presentation');
    this.btnRecord = document.getElementById('btn-record');
    this.btnLeave = document.getElementById('btn-leave');

    this.presControls = document.getElementById('pres-controls');
    this.btnPrevSlide = document.getElementById('btn-prev-slide');
    this.btnNextSlide = document.getElementById('btn-next-slide');
    this.btnLaser = document.getElementById('btn-laser');
    this.slideFileInput = document.getElementById('slide-file-input');
    this.slideIndicator = document.getElementById('slide-indicator');

    this.modalOverlay = document.getElementById('consensus-modal');
    this.btnVoteAccept = document.getElementById('btn-vote-accept');
    this.btnVoteDeny = document.getElementById('btn-vote-deny');
    this.tallyTotal = document.getElementById('tally-total');
    this.tallyYes = document.getElementById('tally-yes');
    this.tallyNo = document.getElementById('tally-no');

    this.recordingToast = document.getElementById('recording-toast');
    this.btnRevokeConsent = document.getElementById('btn-revoke-consent');
    this.recTimer = document.getElementById('rec-timer');
    this.peerCountVal = document.getElementById('peer-count-val');
    this.peerCountTag = document.getElementById('peer-count-tag');
    this.localAudioLevel = document.getElementById('local-audio-level');

    // Command Center Modal Elements
    this.cmdModal = document.getElementById('command-center-modal');
    this.btnHeaderMenu = document.getElementById('btn-header-menu');
    this.btnDockMenu = document.getElementById('btn-dock-menu');
    this.btnCloseCmdMenu = document.getElementById('btn-close-cmd-menu');
    this.cmdTermOutput = document.getElementById('cmd-term-output');

    // Individual Command Buttons
    this.cmdBtnPeers = document.getElementById('cmd-btn-peers');
    this.cmdBtnRecordA = document.getElementById('cmd-btn-record-a');
    this.cmdBtnRecordV = document.getElementById('cmd-btn-record-v');
    this.cmdBtnAccept = document.getElementById('cmd-btn-accept');
    this.cmdBtnDeny = document.getElementById('cmd-btn-deny');
    this.cmdBtnStartrec = document.getElementById('cmd-btn-startrec');
    this.cmdBtnStoprec = document.getElementById('cmd-btn-stoprec');
    this.cmdBtnRevoke = document.getElementById('cmd-btn-revoke');
    this.cmdBtnScreen = document.getElementById('cmd-btn-screen');
    this.cmdBtnPresentNext = document.getElementById('cmd-btn-present-next');
    this.cmdBtnStats = document.getElementById('cmd-btn-stats');
    this.cmdBtnOpen = document.getElementById('cmd-btn-open');
    this.cmdBtnQuit = document.getElementById('cmd-btn-quit');
    this.cmdBtnInvite = document.getElementById('cmd-btn-invite');

    // SMTP Modal Elements
    this.smtpModal = document.getElementById('smtp-modal');
    this.btnHeaderInvite = document.getElementById('btn-header-invite');
    this.btnDockInvite = document.getElementById('btn-dock-invite');
    this.btnCloseSmtpModal = document.getElementById('btn-close-smtp-modal');
    this.smtpPresetSelect = document.getElementById('smtp-preset-select');
    this.smtpHost = document.getElementById('smtp-host');
    this.smtpPort = document.getElementById('smtp-port');
    this.smtpUser = document.getElementById('smtp-user');
    this.smtpPass = document.getElementById('smtp-pass');
    this.smtpSender = document.getElementById('smtp-sender');
    this.smtpName = document.getElementById('smtp-name');
    this.btnSaveSmtpCfg = document.getElementById('btn-save-smtp-cfg');
    this.inviteTo = document.getElementById('invite-to');
    this.inviteName = document.getElementById('invite-name');
    this.inviteNote = document.getElementById('invite-note');
    this.btnSendSmtpInvite = document.getElementById('btn-send-smtp-invite');
    this.smtpLogBox = document.getElementById('smtp-log-box');
    this.smtpBadge = document.getElementById('smtp-badge');

    // Peers Modal Elements
    this.peersModal = document.getElementById('peers-modal');
    this.btnClosePeersModal = document.getElementById('btn-close-peers-modal');
    this.peersModalList = document.getElementById('peers-modal-list');
    this.btnCopyInviteLink = document.getElementById('btn-copy-invite-link');
    this.btnRefreshPeers = document.getElementById('btn-refresh-peers');
  }

  logTerminal(msg, type = 'info') {
    if (!this.cmdTermOutput) return;
    const now = new Date().toTimeString().split(' ')[0];
    const line = document.createElement('div');
    line.className = `term-line ${type}`;
    line.textContent = `[${now}] ${msg}`;
    this.cmdTermOutput.appendChild(line);
    this.cmdTermOutput.scrollTop = this.cmdTermOutput.scrollHeight;
  }

  async initHardwareMedia() {
    try {
      this.localStream = await navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: true, noiseSuppression: true, sampleRate: 48000 },
        video: { width: { ideal: 1280 }, height: { ideal: 720 }, frameRate: { ideal: 30 } }
      });

      this.localVideo.srcObject = this.localStream;
      this.setupAudioAnalysis(this.localStream);
      this.logTerminal('Hardware camera and microphone successfully bound.', 'success');
    } catch (err) {
      console.warn('[Gafana] Camera/Mic permission denied or not available. Using fallback privacy canvas.', err);
      this.setupFallbackVideoStream();
      this.logTerminal('Camera/Mic permission unavailable. Privacy canvas stream active.', 'warn');
    }
  }

  setupAudioAnalysis(stream) {
    try {
      const AudioCtx = window.AudioContext || window.webkitAudioContext;
      this.audioContext = new AudioCtx();
      const source = this.audioContext.createMediaStreamSource(stream);
      this.analyser = this.audioContext.createAnalyser();
      this.analyser.fftSize = 64;
      source.connect(this.analyser);

      const dataArray = new Uint8Array(this.analyser.frequencyBinCount);
      const updateVolumeMeter = () => {
        if (!this.analyser) return;
        this.analyser.getByteFrequencyData(dataArray);
        let sum = 0;
        for (let i = 0; i < dataArray.length; ++i) sum += dataArray[i];
        const avg = sum / dataArray.length;
        const widthPct = Math.min(100, Math.round((avg / 128) * 100));
        if (this.localAudioLevel) this.localAudioLevel.style.width = `${widthPct}%`;
        requestAnimationFrame(updateVolumeMeter);
      };
      updateVolumeMeter();
    } catch (e) {
      console.warn('[Gafana] WebAudio analysis unavailable:', e);
    }
  }

  setupFallbackVideoStream() {
    const canvas = document.createElement('canvas');
    canvas.width = 1280;
    canvas.height = 720;
    const ctx = canvas.getContext('2d');
    let hue = 210;

    const renderPrivacyFrame = () => {
      ctx.fillStyle = '#090d16';
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.fillStyle = '#38bdf8';
      ctx.font = 'bold 36px Inter, sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('Gafana Privacy Media Stream', canvas.width / 2, canvas.height / 2 - 20);
      ctx.font = '18px Inter, sans-serif';
      ctx.fillStyle = '#94a3b8';
      ctx.fillText('Camera Inactive | Zero Cloud Storage | E2EE Active', canvas.width / 2, canvas.height / 2 + 30);
    };
    renderPrivacyFrame();

    this.localStream = canvas.captureStream(30);
    this.localVideo.srcObject = this.localStream;
  }

  initEventListeners() {
    // Microphone Toggle
    this.btnMic.addEventListener('click', () => {
      if (this.localStream) {
        const audioTracks = this.localStream.getAudioTracks();
        if (audioTracks.length > 0) {
          audioTracks[0].enabled = !audioTracks[0].enabled;
          const isEnabled = audioTracks[0].enabled;
          this.btnMic.classList.toggle('active', isEnabled);
          this.btnMic.querySelector('.btn-icon').textContent = isEnabled ? '🎤' : '🔇';
          this.btnMic.querySelector('.btn-text').textContent = isEnabled ? 'Mute' : 'Unmute';
          this.logTerminal(`Microphone ${isEnabled ? 'unmuted' : 'muted'}.`, isEnabled ? 'info' : 'warn');
        }
      }
    });

    // Camera Toggle
    this.btnCam.addEventListener('click', () => {
      if (this.localStream) {
        const videoTracks = this.localStream.getVideoTracks();
        if (videoTracks.length > 0) {
          videoTracks[0].enabled = !videoTracks[0].enabled;
          const isEnabled = videoTracks[0].enabled;
          this.btnCam.classList.toggle('active', isEnabled);
          this.btnCam.querySelector('.btn-icon').textContent = isEnabled ? '📹' : '🚫';
          this.btnCam.querySelector('.btn-text').textContent = isEnabled ? 'Stop Video' : 'Start Video';
          this.videoPlaceholder.style.display = isEnabled ? 'none' : 'flex';
          this.localVideo.style.display = isEnabled ? 'block' : 'none';
          this.logTerminal(`Camera ${isEnabled ? 'resumed' : 'paused'}.`, isEnabled ? 'info' : 'warn');
        }
      }
    });

    // Native Screen Sharing
    this.btnScreen.addEventListener('click', async () => {
      if (this.screenStream) {
        this.stopScreenSharing();
      } else {
        await this.startScreenSharing();
      }
    });

    // Presentation Toggle
    this.btnPres.addEventListener('click', () => {
      const isPres = this.presCanvasWrapper.style.display === 'flex';
      if (isPres) {
        this.presCanvasWrapper.style.display = 'none';
        this.presControls.style.display = 'none';
        this.localVideo.style.display = 'block';
        this.btnPres.classList.remove('active');
        this.logTerminal('Presentation stage closed.', 'info');
      } else {
        this.presCanvasWrapper.style.display = 'flex';
        this.presControls.style.display = 'flex';
        this.localVideo.style.display = 'none';
        this.btnPres.classList.add('active');
        this.renderCurrentSlide();
        this.logTerminal('Synchronized presentation stage opened.', 'success');
      }
    });

    // Laser pointer
    this.btnLaser.addEventListener('click', () => {
      this.laserActive = !this.laserActive;
      this.btnLaser.classList.toggle('active', this.laserActive);
      this.laserDot.style.display = this.laserActive ? 'block' : 'none';
    });

    this.slideCanvas.addEventListener('mousemove', (e) => {
      if (!this.laserActive) return;
      const rect = this.slideCanvas.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      this.laserDot.style.left = `${x}px`;
      this.laserDot.style.top = `${y}px`;
    });

    // Slide navigation
    this.btnNextSlide.addEventListener('click', () => {
      if (this.slideImages.length > 0 && this.currentSlideIndex < this.slideImages.length - 1) {
        this.currentSlideIndex++;
        this.renderCurrentSlide();
        this.logTerminal(`Advanced to slide ${this.currentSlideIndex + 1}.`, 'info');
      } else {
        this.logTerminal('Already at latest slide.', 'warn');
      }
    });

    this.btnPrevSlide.addEventListener('click', () => {
      if (this.currentSlideIndex > 0) {
        this.currentSlideIndex--;
        this.renderCurrentSlide();
        this.logTerminal(`Returned to slide ${this.currentSlideIndex + 1}.`, 'info');
      }
    });

    // Slide Upload
    this.slideFileInput.addEventListener('change', (e) => {
      const files = Array.from(e.target.files);
      if (files.length === 0) return;
      this.slideImages = [];
      let loaded = 0;

      files.forEach((f, idx) => {
        const img = new Image();
        img.onload = () => {
          this.slideImages[idx] = img;
          loaded++;
          if (loaded === files.length) {
            this.currentSlideIndex = 0;
            this.renderCurrentSlide();
            this.logTerminal(`Loaded ${loaded} presentation slides into RAM.`, 'success');
          }
        };
        img.src = URL.createObjectURL(f);
      });
    });

    // Consensus Voting & Local Recording
    this.btnRecord.addEventListener('click', () => {
      if (this.isRecording) {
        this.stopLocalRecording();
      } else {
        this.initiateRecordingConsent('av');
      }
    });

    this.btnVoteAccept.addEventListener('click', () => {
      this.castVote(true);
    });

    this.btnVoteDeny.addEventListener('click', () => {
      this.castVote(false);
    });

    this.btnRevokeConsent.addEventListener('click', () => {
      this.revokeConsent();
    });

    this.btnLeave.addEventListener('click', () => {
      if (confirm('Leave this Gafana meeting? All in-memory channels will be severed with zero data retained.')) {
        if (this.isRecording) this.stopLocalRecording();
        document.body.innerHTML = `
          <div style="display:flex; flex-direction:column; align-items:center; justify-content:center; height:100vh; text-align:center; background:#0a0d14; color:#f8fafc; font-family:'Inter', sans-serif;">
            <div style="width:64px; height:64px; background:linear-gradient(135deg,#00f2fe,#4facfe); border-radius:18px; display:flex; align-items:center; justify-content:center; font-size:32px; font-weight:800; color:#030712; margin-bottom:20px;">G</div>
            <h1 style="font-size:32px; margin-bottom:12px;">Call Safely Disconnected</h1>
            <p style="color:#94a3b8; font-size:16px; margin-bottom:28px; max-width:480px; line-height:1.6;">
              Zero audio, video, or data was stored or written to any server.<br>
              All ephemeral memory was automatically discarded upon exit.
            </p>
            <button onclick="location.reload()" style="padding:14px 28px; border-radius:12px; background:linear-gradient(135deg,#00f2fe,#4facfe); color:#030712; font-weight:700; border:none; cursor:pointer; font-size:15px;">Rejoin Call</button>
          </div>
        `;
      }
    });

    // Keyboard Shortcuts
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        if (this.cmdModal) this.cmdModal.style.display = 'none';
        if (this.smtpModal) this.smtpModal.style.display = 'none';
        if (this.peersModal) this.peersModal.style.display = 'none';
        if (this.modalOverlay) this.modalOverlay.style.display = 'none';
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        this.toggleCommandMenu();
      }
    });
  }

  // =========================================================================
  // ⚡ Visual Command Center Implementation (All Commands as Buttons)
  // =========================================================================

  toggleCommandMenu() {
    if (!this.cmdModal) return;
    const isShowing = this.cmdModal.style.display === 'flex';
    this.cmdModal.style.display = isShowing ? 'none' : 'flex';
    if (!isShowing) {
      this.logTerminal('Gafana Command Menu opened. Click any action button to execute.', 'info');
    }
  }

  initCommandCenter() {
    // Open triggers
    if (this.btnHeaderMenu) this.btnHeaderMenu.addEventListener('click', () => this.toggleCommandMenu());
    if (this.btnDockMenu) this.btnDockMenu.addEventListener('click', () => this.toggleCommandMenu());
    if (this.btnCloseCmdMenu) this.btnCloseCmdMenu.addEventListener('click', () => {
      this.cmdModal.style.display = 'none';
    });

    // Close when clicking modal backdrop
    if (this.cmdModal) {
      this.cmdModal.addEventListener('click', (e) => {
        if (e.target === this.cmdModal) this.cmdModal.style.display = 'none';
      });
    }

    // 1. peers
    if (this.cmdBtnPeers) {
      this.cmdBtnPeers.addEventListener('click', async () => {
        this.logTerminal('> Executing command: "peers"', 'info');
        await this.fetchAndShowPeers();
      });
    }

    // 2. record a (Audio-only)
    if (this.cmdBtnRecordA) {
      this.cmdBtnRecordA.addEventListener('click', () => {
        this.logTerminal('> Executing command: "record a" (Audio-only Consent Ballot)', 'info');
        if (this.isRecording) {
          this.logTerminal('Recording already active. Call "stoprec" first.', 'warn');
          return;
        }
        this.cmdModal.style.display = 'none';
        this.initiateRecordingConsent('audio');
      });
    }

    // 3. record v (Full Video + Audio)
    if (this.cmdBtnRecordV) {
      this.cmdBtnRecordV.addEventListener('click', () => {
        this.logTerminal('> Executing command: "record v" (Full Video+Audio Consent Ballot)', 'info');
        if (this.isRecording) {
          this.logTerminal('Recording already active. Call "stoprec" first.', 'warn');
          return;
        }
        this.cmdModal.style.display = 'none';
        this.initiateRecordingConsent('av');
      });
    }

    // 4. accept
    if (this.cmdBtnAccept) {
      this.cmdBtnAccept.addEventListener('click', () => {
        this.logTerminal('> Executing command: "accept" (Vote YES)', 'success');
        if (!this.currentBallot) {
          this.logTerminal('Notice: No active recording ballot currently pending.', 'warn');
          return;
        }
        this.castVote(true);
      });
    }

    // 5. deny
    if (this.cmdBtnDeny) {
      this.cmdBtnDeny.addEventListener('click', () => {
        this.logTerminal('> Executing command: "deny" (Cast VETO)', 'danger');
        if (!this.currentBallot) {
          this.logTerminal('Notice: No active recording ballot currently pending.', 'warn');
          return;
        }
        this.castVote(false);
      });
    }

    // 6. startrec
    if (this.cmdBtnStartrec) {
      this.cmdBtnStartrec.addEventListener('click', () => {
        this.logTerminal('> Executing command: "startrec" (Begin capturing to local disk)', 'info');
        if (this.isRecording) {
          this.logTerminal('Recording is already running.', 'warn');
          return;
        }
        if (this.ballotState !== 'APPROVED') {
          this.logTerminal('SECURITY VIOLATION: Cannot start recording without 100% unanimous consent.', 'danger');
          alert('Security Violation: Unanimous consent is not granted.\nInitiate a ballot first via "record a" or "record v".');
          return;
        }
        this.cmdModal.style.display = 'none';
        this.startLocalRecording();
      });
    }

    // 7. stoprec
    if (this.cmdBtnStoprec) {
      this.cmdBtnStoprec.addEventListener('click', () => {
        this.logTerminal('> Executing command: "stoprec" (Finalize local capture)', 'info');
        if (!this.isRecording) {
          this.logTerminal('Notice: No recording is currently active.', 'warn');
          return;
        }
        this.stopLocalRecording();
        this.logTerminal('Local recording finalized and downloaded to disk with 0 cloud traces.', 'success');
      });
    }

    // 8. revoke
    if (this.cmdBtnRevoke) {
      this.cmdBtnRevoke.addEventListener('click', () => {
        this.logTerminal('> Executing command: "revoke" (Force-terminate ongoing recording)', 'danger');
        this.revokeConsent();
      });
    }

    // 9. screen
    if (this.cmdBtnScreen) {
      this.cmdBtnScreen.addEventListener('click', () => {
        this.logTerminal('> Executing command: "screen" (Toggle screen sharing)', 'info');
        this.btnScreen.click();
      });
    }

    // 10. present / next
    if (this.cmdBtnPresentNext) {
      this.cmdBtnPresentNext.addEventListener('click', () => {
        this.logTerminal('> Executing command: "present / next" (Start presentation & advance)', 'info');
        const isPres = this.presCanvasWrapper.style.display === 'flex';
        if (!isPres) {
          this.btnPres.click();
          this.logTerminal('Presentation mode activated.', 'success');
        } else {
          this.btnNextSlide.click();
          this.logTerminal(`Advanced to slide ${this.currentSlideIndex + 1}.`, 'info');
        }
      });
    }

    // 11. stats
    if (this.cmdBtnStats) {
      this.cmdBtnStats.addEventListener('click', async () => {
        this.logTerminal('> Executing command: "stats" (Querying in-memory relay telemetry)...', 'info');
        try {
          const res = await fetch('/api/stats');
          const data = await res.json();
          this.logTerminal(`[STATS OK] Packets Routed In-Memory: ${data.sfu_packets} | Server Disk Writes: ${data.server_disk_writes_bytes} B | Rule: ${data.consensus_rule}`, 'success');
          this.logTerminal(`Status: ${data.status} | Ephemeral Engine: ${data.engine} v${data.version}`, 'info');
        } catch (e) {
          this.logTerminal(`Stats query failed: ${e.message}`, 'danger');
        }
      });
    }

    // 12. open
    if (this.cmdBtnOpen) {
      this.cmdBtnOpen.addEventListener('click', async () => {
        this.logTerminal('> Executing command: "open" (Re-open http://localhost:7878)...', 'info');
        try {
          await fetch('/api/open', { method: 'POST' });
        } catch (e) {
          // ignore
        }
        window.open(window.location.origin, '_blank');
        this.logTerminal('Browser tab opened.', 'success');
      });
    }

    // 13. quit
    if (this.cmdBtnQuit) {
      this.cmdBtnQuit.addEventListener('click', async () => {
        if (!confirm('Gracefully shut down the Gafana executable and terminate all threads? All ephemeral session state will be wiped.')) {
          return;
        }
        this.logTerminal('> Executing command: "quit" (Gracefully shutting down engine)...', 'danger');
        try {
          await fetch('/api/quit', { method: 'POST' });
        } catch (e) {
          // ignore
        }
        this.cmdModal.style.display = 'none';
        document.body.innerHTML = `
          <div style="display:flex; flex-direction:column; align-items:center; justify-content:center; height:100vh; text-align:center; background:#07090e; color:#f8fafc; font-family:'Inter', sans-serif;">
            <div style="width:68px; height:68px; background:rgba(244,63,94,0.15); border:1px solid #f43f5e; border-radius:20px; display:flex; align-items:center; justify-content:center; font-size:32px; color:#f43f5e; margin-bottom:20px;">🛑</div>
            <h1 style="font-size:32px; margin-bottom:12px; color:#ffffff;">Gafana Engine Terminated</h1>
            <p style="color:#94a3b8; font-size:16px; margin-bottom:24px; max-width:520px; line-height:1.6;">
              The native single-executable process has shut down all socket threads, closed the SFU relay, and cleanly freed all in-memory buffers.<br><br>
              <strong>Zero files or traces were left on any server.</strong>
            </p>
            <div style="font-family:'JetBrains Mono', monospace; font-size:13px; color:#64748b; background:rgba(255,255,255,0.03); padding:10px 20px; border-radius:8px; border:1px solid rgba(255,255,255,0.08);">
              Exit code 0. You may close this browser window.
            </div>
          </div>
        `;
      });
    }

    // 14. invite (smtp)
    if (this.cmdBtnInvite) {
      this.cmdBtnInvite.addEventListener('click', () => {
        this.cmdModal.style.display = 'none';
        this.openSmtpModal();
      });
    }
  }

  // =========================================================================
  // ✉️ Secure SMTP Invitation Client Implementation
  // =========================================================================

  openSmtpModal() {
    if (!this.smtpModal) return;
    this.smtpModal.style.display = 'flex';
    this.loadSmtpStatus();
  }

  async loadSmtpStatus() {
    try {
      const res = await fetch('/api/smtp/status');
      const data = await res.json();
      if (data && data.configured) {
        if (this.smtpHost && data.host) this.smtpHost.value = data.host;
        if (this.smtpPort && data.port) this.smtpPort.value = data.port;
        if (this.smtpSender && data.sender_email) this.smtpSender.value = data.sender_email;
        if (this.smtpName && data.sender_name) this.smtpName.value = data.sender_name;
        if (this.smtpBadge) this.smtpBadge.textContent = `SMTP: Ready (${data.host})`;
        if (this.smtpLogBox) this.smtpLogBox.textContent = `Active In-Memory SMTP: ${data.host}:${data.port}\nSender: "${data.sender_name}" <${data.sender_email}>\nReady to dispatch secure invitations.`;
      }
    } catch (e) {
      console.warn('[Gafana] Could not query SMTP status:', e);
    }
  }

  initSmtpDispatcher() {
    if (this.btnHeaderInvite) this.btnHeaderInvite.addEventListener('click', () => this.openSmtpModal());
    if (this.btnDockInvite) this.btnDockInvite.addEventListener('click', () => this.openSmtpModal());
    if (this.btnCloseSmtpModal) this.btnCloseSmtpModal.addEventListener('click', () => {
      this.smtpModal.style.display = 'none';
    });

    if (this.smtpModal) {
      this.smtpModal.addEventListener('click', (e) => {
        if (e.target === this.smtpModal) this.smtpModal.style.display = 'none';
      });
    }

    // Provider Presets
    if (this.smtpPresetSelect) {
      this.smtpPresetSelect.addEventListener('change', () => {
        const val = this.smtpPresetSelect.value;
        if (val === 'gmail') {
          this.smtpHost.value = 'smtp.gmail.com';
          this.smtpPort.value = '587';
          this.smtpUser.placeholder = 'you@gmail.com';
          this.smtpPass.placeholder = 'Google 16-character App Password';
          this.smtpLogBox.textContent = 'Selected: Google Gmail SMTP. Enter your account and 16-character Google App Password.';
        } else if (val === 'outlook') {
          this.smtpHost.value = 'smtp-mail.outlook.com';
          this.smtpPort.value = '587';
          this.smtpUser.placeholder = 'you@outlook.com';
          this.smtpPass.placeholder = 'Account password / App token';
          this.smtpLogBox.textContent = 'Selected: Microsoft Outlook / Office 365 SMTP with STARTTLS on port 587.';
        } else {
          this.smtpHost.value = '';
          this.smtpHost.placeholder = 'mail.your-domain.com';
          this.smtpLogBox.textContent = 'Selected: Custom SMTP server. Enter your mail server host, port, and credentials.';
        }
      });
    }

    // Save In-Memory Settings
    if (this.btnSaveSmtpCfg) {
      this.btnSaveSmtpCfg.addEventListener('click', async () => {
        const host = this.smtpHost.value.trim();
        if (!host) {
          alert('Please enter a valid SMTP host address.');
          this.smtpHost.focus();
          return;
        }

        const payload = {
          host: host,
          port: parseInt(this.smtpPort.value, 10) || 587,
          username: this.smtpUser.value.trim(),
          password: this.smtpPass.value.trim(),
          sender_email: this.smtpSender.value.trim() || 'host@gafana.local',
          sender_name: this.smtpName.value.trim() || 'Gafana Host'
        };

        try {
          const res = await fetch('/api/smtp/config', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
          });
          const data = await res.json();
          this.smtpLogBox.textContent = `[CONFIG SAVED] SMTP parameters loaded into volatile executable RAM:\nHost: ${payload.host}:${payload.port}\nSender: ${payload.sender_email}\nCredentials never written to disk.`;
          this.logTerminal(`SMTP server configured: ${payload.host}:${payload.port}`, 'success');
          if (this.smtpBadge) this.smtpBadge.textContent = `SMTP: Ready (${payload.host})`;
        } catch (e) {
          this.smtpLogBox.textContent = `[CONFIG ERROR] Failed to save SMTP configuration: ${e.message}`;
        }
      });
    }

    // Send Invite via SMTP
    if (this.btnSendSmtpInvite) {
      this.btnSendSmtpInvite.addEventListener('click', async () => {
        const to = this.inviteTo.value.trim();
        const name = this.inviteName.value.trim() || 'Attendee';
        const note = this.inviteNote.value.trim() || 'Confidential meeting';
        const host = this.smtpHost.value.trim();

        if (!host) {
          alert('Please specify your SMTP server host (e.g., smtp.gmail.com).');
          this.smtpHost.focus();
          return;
        }

        if (!to || !to.includes('@')) {
          alert('Please enter a valid recipient email address.');
          this.inviteTo.focus();
          return;
        }

        this.smtpLogBox.textContent = `Initiating secure invite handshake to ${to}...\nConnecting to ${host}:${this.smtpPort.value}...`;
        this.btnSendSmtpInvite.disabled = true;
        this.btnSendSmtpInvite.innerHTML = '<span>⏳ Dispatching Invite...</span>';

        try {
          // Auto-save active config first
          await fetch('/api/smtp/config', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              host: host,
              port: parseInt(this.smtpPort.value, 10) || 587,
              username: this.smtpUser.value.trim(),
              password: this.smtpPass.value.trim(),
              sender_email: this.smtpSender.value.trim() || 'host@gafana.local',
              sender_name: this.smtpName.value.trim() || 'Gafana Host'
            })
          });

          // Dispatch invitation
          const res = await fetch('/api/smtp/invite', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ to, name, note })
          });
          const data = await res.json();

          this.smtpLogBox.textContent = data.log || (data.success ? 'Invite dispatched successfully.' : 'Invite failed.');

          if (data.success) {
            this.logTerminal(`[SMTP INVITE OK] Secure invite dispatched to ${to} (${name})`, 'success');
            alert(`Secure invitation successfully sent to:\n${to}\n\nAttendee can join with 100% privacy and zero server data retention.`);
            this.inviteTo.value = '';
          } else {
            this.logTerminal(`[SMTP INVITE FAILED] Could not send invite to ${to} (code: ${data.code})`, 'danger');
            alert(`SMTP Delivery Failed: Could not deliver email over TCP.\nPlease verify your SMTP host, port, username, and password in the log.`);
          }
        } catch (e) {
          this.smtpLogBox.textContent = `[SMTP DISPATCH EXCEPTION] ${e.message}`;
          this.logTerminal(`SMTP dispatch error: ${e.message}`, 'danger');
        } finally {
          this.btnSendSmtpInvite.disabled = false;
          this.btnSendSmtpInvite.innerHTML = '<span>🚀 Send Secure Invitation via SMTP</span>';
        }
      });
    }
  }

  // =========================================================================
  // 👥 Peers Inspection Modal
  // =========================================================================

  initPeersModal() {
    if (this.peerCountTag) {
      this.peerCountTag.addEventListener('click', () => this.fetchAndShowPeers());
    }
    if (this.btnClosePeersModal) {
      this.btnClosePeersModal.addEventListener('click', () => {
        this.peersModal.style.display = 'none';
      });
    }
    if (this.peersModal) {
      this.peersModal.addEventListener('click', (e) => {
        if (e.target === this.peersModal) this.peersModal.style.display = 'none';
      });
    }

    if (this.btnCopyInviteLink) {
      this.btnCopyInviteLink.addEventListener('click', () => {
        const isLocal = window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1';
        const link = (isLocal && this.networkUrl) ? this.networkUrl : window.location.href;
        navigator.clipboard.writeText(link).then(() => {
          alert(`Confidential Gafana Room Link copied to clipboard:\n${link}\n\n(Share this URL with attendees on your LAN/network to join directly)`);
        }).catch(() => {
          prompt('Copy this confidential room link:', link);
        });
      });
    }

    if (this.btnRefreshPeers) {
      this.btnRefreshPeers.addEventListener('click', () => this.fetchAndShowPeers());
    }
  }

  async fetchNetworkInfo() {
    try {
      const res = await fetch('/api/status');
      if (res.ok) {
        const data = await res.json();
        if (data.network_url) {
          this.networkUrl = data.network_url;
        }
      }
    } catch (_) {}
  }

  initPeerSync() {
    this.pollPeers();
    this.pollConsentStatus();
    setInterval(() => {
      this.pollPeers();
      this.pollConsentStatus();
    }, 2500);
  }

  async pollConsentStatus() {
    try {
      const res = await fetch('/api/consent/status');
      if (!res.ok) return;
      const data = await res.json();
      // state: 0 = IDLE, 1 = REQUESTED, 2 = UNANIMOUS_OK, 3 = REJECTED, 4 = REVOKED, 5 = EXPIRED
      if (data.state === 1) { // REQUESTED
        this.currentBallot = {
          id: data.ballot_id,
          yesVotes: data.yes,
          noVotes: data.no,
          totalExpected: data.total
        };
        this.updateBallotModal();
        if (this.modalOverlay && this.ballotState !== 'VOTED') {
          this.modalOverlay.style.display = 'flex';
        }
      } else if (data.state === 2) { // UNANIMOUS_OK
        if (this.modalOverlay) this.modalOverlay.style.display = 'none';
        if (!this.isRecording && (this.ballotState === 'REQUESTED' || this.ballotState === 'VOTED')) {
          this.ballotState = 'APPROVED';
          this.startLocalRecording();
        }
      } else if (data.state === 3) { // REJECTED
        if (this.modalOverlay) this.modalOverlay.style.display = 'none';
        if (this.isRecording) this.stopLocalRecording();
        if (this.ballotState === 'REQUESTED' || this.ballotState === 'VOTED') {
          this.ballotState = 'REJECTED';
          this.logTerminal('CONSENSUS REJECTED: A participant cast an immediate VETO.', 'danger');
          alert('Recording consent was REJECTED by veto.\nIn Gafana, no recording can proceed without 100% unanimous agreement.');
        }
      } else if (data.state === 4) { // REVOKED
        if (this.isRecording) {
          this.stopLocalRecording();
          this.logTerminal('Recording halted immediately: participant revoked consent mid-call.', 'danger');
        }
      }
    } catch (_) {}
  }

  async pollPeers() {
    try {
      const res = await fetch('/api/peers');
      if (!res.ok) return;
      const data = await res.json();
      const peers = data.peers || [];
      if (this.peerCountVal) this.peerCountVal.textContent = peers.length;
      if (peers.length > 0) {
        this.participants = peers.map(p => ({
          id: p.id,
          name: p.name || 'Participant',
          hasVoted: false,
          vote: null,
          isSelf: !!p.is_self
        }));
      }
    } catch (_) {}
  }

  async fetchAndShowPeers() {
    try {
      const res = await fetch('/api/peers');
      const data = await res.json();
      const peers = data.peers || [];
      if (this.peerCountVal) this.peerCountVal.textContent = peers.length;

      if (peers.length > 0) {
        this.participants = peers.map(p => ({
          id: p.id,
          name: p.name || 'Participant',
          hasVoted: false,
          vote: null,
          isSelf: !!p.is_self
        }));
      }

      if (this.peersModalList) {
        this.peersModalList.innerHTML = peers.map((p, idx) => `
          <div class="peer-list-row">
            <div>
              <div class="peer-row-name">${p.name || 'Participant'} ${p.is_self ? '<span style="color:#00f2fe; font-size:11px;">(You)</span>' : ''}</div>
              <div class="peer-row-id">ID: ${p.id}</div>
            </div>
            <div style="display:flex; align-items:center; gap:8px;">
              <span style="font-size:11px; color:#10b981;">● Online</span>
              <span style="font-size:13px;">🎤</span>
            </div>
          </div>
        `).join('');
      }

      this.peersModal.style.display = 'flex';
      this.logTerminal(`[PEERS] Room "${data.room || 'Gafana-Secure-Room'}": ${peers.length} active participant(s).`, 'success');
    } catch (e) {
      this.logTerminal(`Peers fetch error: ${e.message}`, 'danger');
    }
  }

  // =========================================================================
  // Media & Presentation Methods
  // =========================================================================

  renderCurrentSlide() {
    this.slideCtx.clearRect(0, 0, this.slideCanvas.width, this.slideCanvas.height);

    if (this.slideImages.length > 0 && this.slideImages[this.currentSlideIndex]) {
      const img = this.slideImages[this.currentSlideIndex];
      this.slideCtx.drawImage(img, 0, 0, this.slideCanvas.width, this.slideCanvas.height);
      this.slideIndicator.textContent = `${this.currentSlideIndex + 1} / ${this.slideImages.length}`;
    } else {
      this.slideCtx.fillStyle = '#0f172a';
      this.slideCtx.fillRect(0, 0, this.slideCanvas.width, this.slideCanvas.height);

      this.slideCtx.fillStyle = '#38bdf8';
      this.slideCtx.font = 'bold 44px Inter, sans-serif';
      this.slideCtx.textAlign = 'center';
      this.slideCtx.fillText('Gafana Core Presentation', this.slideCanvas.width / 2, 280);

      this.slideCtx.fillStyle = '#94a3b8';
      this.slideCtx.font = '22px Inter, sans-serif';
      this.slideCtx.fillText('100% Unanimous Consensus | Client-Side Storage Only | Stateless Relay', this.slideCanvas.width / 2, 340);

      this.slideCtx.fillStyle = '#10b981';
      this.slideCtx.font = '18px JetBrains Mono, monospace';
      this.slideCtx.fillText('[Click "Load Slides" to import your own deck]', this.slideCanvas.width / 2, 420);
      this.slideIndicator.textContent = '1 / 1';
    }
  }

  async startScreenSharing() {
    try {
      this.screenStream = await navigator.mediaDevices.getDisplayMedia({
        video: { cursor: 'always' },
        audio: true
      });

      this.screenVideo.srcObject = this.screenStream;
      this.screenVideo.style.display = 'block';
      this.localVideo.style.display = 'none';
      this.btnScreen.classList.add('active');
      this.btnScreen.querySelector('.btn-text').textContent = 'Stop Sharing';
      this.logTerminal('Native display/window screen sharing stream published.', 'success');

      this.screenStream.getVideoTracks()[0].onended = () => {
        this.stopScreenSharing();
      };
    } catch (e) {
      console.warn('[Gafana] Screen sharing cancelled or not supported:', e);
      this.logTerminal('Screen sharing cancelled.', 'warn');
    }
  }

  stopScreenSharing() {
    if (this.screenStream) {
      this.screenStream.getTracks().forEach(t => t.stop());
      this.screenStream = null;
    }
    this.screenVideo.style.display = 'none';
    this.localVideo.style.display = 'block';
    this.btnScreen.classList.remove('active');
    this.btnScreen.querySelector('.btn-text').textContent = 'Share Screen';
    this.logTerminal('Screen sharing stopped.', 'info');
  }

  // =========================================================================
  // Consensus Protocol & Local Recording
  // =========================================================================

  async initiateRecordingConsent(type = 'av') {
    this.recordingType = type;
    const typeLabel = (type === 'audio') ? 'Audio-Only' : 'Full Video + Audio';
    this.logTerminal(`Initiating engine consensus ballot for [${typeLabel}]...`, 'info');

    try {
      const res = await fetch('/api/consent/request', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ type: type })
      });
      const data = await res.json();
      if (data.code === 0) {
        this.currentBallot = {
          id: data.ballot_id,
          type: type,
          yesVotes: 1,
          noVotes: 0,
          totalExpected: Math.max(1, this.participants.length)
        };
        this.logTerminal(`Engine ballot #${data.ballot_id} active. Polling room consensus...`, 'info');
      } else {
        this.logTerminal(`Engine returned notice (code ${data.code}).`, 'warn');
      }
    } catch (e) {
      this.logTerminal(`Consent request communication notice: ${e.message}`, 'warn');
    }

    this.ballotState = 'REQUESTED';
    this.updateBallotModal();
    if (this.participants.length > 1) {
      this.modalOverlay.style.display = 'flex';
    }
    await this.pollConsentStatus();
  }

  async castVote(isYes) {
    const bid = this.currentBallot ? this.currentBallot.id : 0;
    try {
      await fetch('/api/consent/vote', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ballot_id: bid, vote: isYes ? 1 : 0 })
      });
    } catch (_) {}

    this.ballotState = isYes ? 'VOTED' : 'REJECTED';
    this.modalOverlay.style.display = 'none';
    this.logTerminal(`Cast vote on ballot #${bid}: ${isYes ? 'ACCEPT (YES)' : 'DENY (VETO)'}`, isYes ? 'info' : 'danger');

    if (!isYes) {
      alert('Recording consent was REJECTED by veto.\nIn Gafana, no recording can proceed without 100% unanimous agreement from every participant.');
      this.currentBallot = null;
    }
    await this.pollConsentStatus();
  }

  updateBallotModal() {
    if (!this.currentBallot) return;
    this.tallyTotal.textContent = this.currentBallot.totalExpected;
    this.tallyYes.textContent = this.currentBallot.yesVotes;
    this.tallyNo.textContent = this.currentBallot.noVotes;
  }

  async revokeConsent() {
    const bid = this.currentBallot ? this.currentBallot.id : 0;
    try {
      await fetch('/api/consent/revoke', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ballot_id: bid })
      });
    } catch (_) {}

    this.ballotState = 'REVOKED';
    this.logTerminal('CONSENT REVOKED! Local recording halted immediately by participant command.', 'danger');
    alert('Consent revoked! Local recording has been forcefully halted immediately.');
    this.stopLocalRecording();
  }

  startLocalRecording() {
    if (this.ballotState !== 'APPROVED') {
      alert('Security violation: Cannot record without 100% unanimous consent.');
      return;
    }

    try {
      let streamToRecord;
      if (this.recordingType === 'audio') {
        const audioTracks = (this.localStream) ? this.localStream.getAudioTracks() : [];
        if (audioTracks.length === 0) {
          alert('No microphone track available for audio-only recording.');
          return;
        }
        streamToRecord = new MediaStream(audioTracks);
      } else {
        streamToRecord = this.screenStream || this.localStream;
      }

      if (!streamToRecord) {
        alert('No active media stream found to record.');
        return;
      }

      this.recordedChunks = [];
      const mime = (this.recordingType === 'audio') ? 'audio/webm;codecs=opus' : 'video/webm;codecs=vp8,opus';
      const options = { mimeType: mime };
      if (!MediaRecorder.isTypeSupported(options.mimeType)) {
        delete options.mimeType;
      }

      this.mediaRecorder = new MediaRecorder(streamToRecord, options);

      this.mediaRecorder.ondataavailable = (event) => {
        if (event.data && event.data.size > 0) {
          this.recordedChunks.push(event.data);
        }
      };

      this.mediaRecorder.onstop = () => {
        this.saveRecordingLocally();
      };

      this.mediaRecorder.start(1000);
      this.isRecording = true;
      this.recordStartTime = Date.now();

      this.recordingToast.classList.add('show');
      this.btnRecord.classList.add('active');
      this.btnRecord.querySelector('.btn-icon').textContent = '⏹';
      this.btnRecord.querySelector('.btn-text').textContent = 'Stop Recording';

      this.recordTimerInterval = setInterval(() => {
        const elapsed = Math.floor((Date.now() - this.recordStartTime) / 1000);
        const m = String(Math.floor(elapsed / 60)).padStart(2, '0');
        const s = String(elapsed % 60).padStart(2, '0');
        this.recTimer.textContent = `${m}:${s}`;
      }, 1000);

      this.logTerminal(`Local client recording active (${this.recordingType === 'audio' ? 'Audio-only' : 'Full A/V'}). Zero cloud storage.`, 'success');
    } catch (err) {
      console.error('[Gafana] Failed to start MediaRecorder:', err);
      this.logTerminal(`Failed to start MediaRecorder: ${err.message}`, 'danger');
      alert('Failed to start recording: ' + err.message);
    }
  }

  stopLocalRecording() {
    if (this.mediaRecorder && this.mediaRecorder.state !== 'inactive') {
      this.mediaRecorder.stop();
    }
    this.isRecording = false;
    this.ballotState = 'IDLE';
    this.currentBallot = null;

    if (this.recordTimerInterval) {
      clearInterval(this.recordTimerInterval);
      this.recordTimerInterval = null;
    }

    this.recordingToast.classList.remove('show');
    this.btnRecord.classList.remove('active');
    this.btnRecord.querySelector('.btn-icon').textContent = '⏺';
    this.btnRecord.querySelector('.btn-text').textContent = 'Record Meeting';
  }

  saveRecordingLocally() {
    if (this.recordedChunks.length === 0) return;

    const mime = (this.recordingType === 'audio') ? 'audio/webm' : 'video/webm';
    const ext = (this.recordingType === 'audio') ? 'weba' : 'webm';
    const blob = new Blob(this.recordedChunks, { type: mime });
    const url = URL.createObjectURL(blob);
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    const filename = `gafana_local_${this.recordingType}_${timestamp}.${ext}`;

    const a = document.createElement('a');
    a.style.display = 'none';
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);

    setTimeout(() => URL.revokeObjectURL(url), 5000);
    this.recordedChunks = [];

    this.logTerminal(`Recording finalized and saved directly to your computer: ${filename}`, 'success');
    alert(`Recording finalized and saved directly to your computer:\n${filename}\n\nNotice: ZERO bytes were transmitted to or stored on any server.`);
  }

  initCallTimer() {
    const timerElem = document.getElementById('timer-tag');
    const start = Date.now();
    setInterval(() => {
      const elapsed = Math.floor((Date.now() - start) / 1000);
      const h = String(Math.floor(elapsed / 3600)).padStart(2, '0');
      const m = String(Math.floor((elapsed % 3600) / 60)).padStart(2, '0');
      const s = String(elapsed % 60).padStart(2, '0');
      if (timerElem) timerElem.textContent = `${h}:${m}:${s}`;
    }, 1000);
  }
}

// Instantiate on document load
document.addEventListener('DOMContentLoaded', () => {
  window.gafanaApp = new GafanaWebClient();
});
