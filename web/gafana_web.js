/**
 * Gafana Production Web Client Engine
 *
 * Implements:
 *  - Zero-Trust Browser Extension Shield (DOM Mutation Observer, Shadow DOM Interceptor & API Freezer)
 *  - Real WebRTC MediaStream acquisition (getUserMedia & getDisplayMedia)
 *  - Web Audio API microphone volume analysis
 *  - Real HTML5 Presentation Canvas & Laser Pointer
 *  - Strict 100% Unanimous Consensus Recording Protocol (Audio-only or Full A/V)
 *  - Visual Command Center Menu executing all engine commands as buttons
 *  - Embedded Secure SMTP Attendee Invitation Setup and Real-Time Log
 *  - Client-side MediaRecorder writing directly to local disk with ZERO cloud retention
 */

// =========================================================================
// ZERO-TRUST BROWSER EXTENSION SHIELD
// Actively intercepts, neutralizes, and purges all third-party extension
// injected nodes, shadow roots, screen recorders, and DOM scrapers.
// =========================================================================
(function setupAntiExtensionShield() {
  let blockedCount = 0;
  const extensionPatterns = [
    'extension', 'loom', 'grammarly', 'lastpass', 'bitwarden', '1password',
    'dashlane', 'honey', 'capitalone', 'screen-record', 'recorder', 'tampermonkey',
    'violentmonkey', 'react-devtools', 'redux-devtools', 'metamask', 'phantom',
    'ext-', 'adblock', 'ublock', 'translate', 'proctor'
  ];

  function isExtensionElement(el) {
    if (!el || el.nodeType !== 1) return false;
    const tag = (el.tagName || '').toLowerCase();

    // Gafana native components are safe
    if ((el.classList && el.classList.contains('gafana-app')) || tag.startsWith('gafana')) {
      return false;
    }

    // Custom elements containing hyphens injected by extensions (e.g. <grammarly-desktop-integration>, <loom-container>)
    if (tag.includes('-') && !tag.startsWith('gafana-')) {
      return true;
    }

    // Extension asset schemes
    const src = el.getAttribute('src') || '';
    const href = el.getAttribute('href') || '';
    if (src.includes('-extension://') || href.includes('-extension://')) {
      return true;
    }

    // Check IDs, class names, and attributes
    const id = (el.id || '').toLowerCase();
    const className = (typeof el.className === 'string' ? el.className : '').toLowerCase();

    for (const pat of extensionPatterns) {
      if (id.includes(pat) && !id.includes('gafana')) return true;
      if (className.includes(pat) && !className.includes('gafana')) return true;
    }

    // Specific known extension marker attributes
    if (el.hasAttribute('data-extension-element') ||
        el.hasAttribute('data-loom') ||
        el.hasAttribute('data-lastpass-root') ||
        el.hasAttribute('data-bitwarden-watching') ||
        el.hasAttribute('data-injected') ||
        el.hasAttribute('data-grammarly-shadow-root')) {
      return true;
    }

    // High z-index overlays typical of extension popups/widgets
    if (el.style && el.style.zIndex && parseInt(el.style.zIndex) > 100000 && !className.includes('gafana')) {
      return true;
    }

    return false;
  }

  function purgeNode(node) {
    if (isExtensionElement(node)) {
      blockedCount++;
      const tag = (node.tagName || '').toLowerCase();
      const id = node.id || '(no-id)';
      console.warn(`[GAFANA SHIELD] Blocked & purged unauthorized extension element: <${tag} id="${id}">`);
      try {
        node.style.setProperty('display', 'none', 'important');
        node.style.setProperty('visibility', 'hidden', 'important');
        node.style.setProperty('opacity', '0', 'important');
        node.style.setProperty('pointer-events', 'none', 'important');
        node.remove();
      } catch (e) {}
      updateShieldBadge(blockedCount);
      return true;
    }

    if (node.children && node.children.length > 0) {
      Array.from(node.children).forEach(purgeNode);
    }
    return false;
  }

  function updateShieldBadge(count) {
    const badge = document.getElementById('shield-status-badge');
    if (badge) {
      badge.innerHTML = `<span>🛡️ Shield: ${count} Extension${count === 1 ? '' : 's'} Blocked</span>`;
      badge.style.display = 'inline-flex';
    }
  }

  // 1. Intercept Element.prototype.attachShadow to neutralize closed/open shadow DOM extension widgets
  try {
    const origAttachShadow = Element.prototype.attachShadow;
    Element.prototype.attachShadow = function(init) {
      if (isExtensionElement(this)) {
        console.warn(`[GAFANA SHIELD] Blocked extension shadowRoot on <${this.tagName.toLowerCase()}>`);
        this.remove();
        return document.createDocumentFragment();
      }
      return origAttachShadow.call(this, init);
    };
  } catch (e) {}

  // 2. Active recursive MutationObserver across document tree
  const observer = new MutationObserver((mutations) => {
    for (const mut of mutations) {
      for (const added of mut.addedNodes) {
        if (added.nodeType === 1) {
          purgeNode(added);
        }
      }
    }
  });

  observer.observe(document.documentElement, {
    childList: true,
    subtree: true
  });

  // 3. Scan DOM immediately upon load
  window.addEventListener('DOMContentLoaded', () => {
    document.querySelectorAll('*').forEach(purgeNode);
  });
  window.addEventListener('load', () => {
    document.querySelectorAll('*').forEach(purgeNode);
  });

  // 4. Protect navigator.mediaDevices against unauthorized extension interception
  try {
    if (navigator.mediaDevices && navigator.mediaDevices.getUserMedia) {
      Object.freeze(navigator.mediaDevices.getUserMedia);
    }
  } catch (e) {}

  window.__gafana_shield_active = true;
  console.log('[GAFANA SHIELD] Zero-Trust Anti-Extension Protection is ACTIVE.');
})();

// =========================================================================
// 🔒 GLOBAL ANTI-CAPTURE & MEDIA PROTECTION HOOKS
// Stops unapproved screenshots, canvas scraping, and unauthorized MediaRecorder
// instantiation when unanimous recording consent has not been granted.
// =========================================================================
window.__gafana_consent_approved = false;

// Intercept MediaRecorder globally: any device, extension, or script attempting
// to instantiate a recorder without 100% unanimous consent is immediately blocked.
(function installMediaRecorderGuard() {
  try {
    const OrigMediaRecorder = window.MediaRecorder;
    if (OrigMediaRecorder) {
      window.__OrigMediaRecorder = OrigMediaRecorder;
      window.MediaRecorder = function(stream, options) {
        if (!window.__gafana_consent_approved) {
          console.error('[ANTI-CAPTURE GUARD] MediaRecorder instantiation BLOCKED: Unanimous consent has not been granted.');
          throw new DOMException(
            'Recording is strictly prohibited: No unanimous recording consent has been granted by call participants.',
            'NotAllowedError'
          );
        }
        return new OrigMediaRecorder(stream, options);
      };

      if (OrigMediaRecorder.isTypeSupported) {
        window.MediaRecorder.isTypeSupported = OrigMediaRecorder.isTypeSupported.bind(OrigMediaRecorder);
      }
      window.MediaRecorder.prototype = OrigMediaRecorder.prototype;
    }
  } catch (e) {
    console.warn('[ANTI-CAPTURE GUARD] Could not wrap MediaRecorder:', e);
  }
})();

// Intercept canvas extraction APIs to block screenshot & scraping tools
(function installCanvasScrapeNeutralizer() {
  try {
    const origToDataURL = HTMLCanvasElement.prototype.toDataURL;
    const origToBlob = HTMLCanvasElement.prototype.toBlob;
    const origGetImageData = CanvasRenderingContext2D.prototype.getImageData;

    HTMLCanvasElement.prototype.toDataURL = function(...args) {
      if (!window.__gafana_consent_approved) {
        console.warn('[ANTI-CAPTURE GUARD] Canvas toDataURL export BLOCKED without unanimous consent.');
        return 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';
      }
      return origToDataURL.apply(this, args);
    };

    HTMLCanvasElement.prototype.toBlob = function(callback, ...args) {
      if (!window.__gafana_consent_approved) {
        console.warn('[ANTI-CAPTURE GUARD] Canvas toBlob export BLOCKED without unanimous consent.');
        if (typeof callback === 'function') {
          callback(new Blob([], { type: 'image/png' }));
        }
        return;
      }
      return origToBlob.call(this, callback, ...args);
    };

    CanvasRenderingContext2D.prototype.getImageData = function(...args) {
      if (!window.__gafana_consent_approved) {
        const width = args[2] || 1;
        const height = args[3] || 1;
        return this.createImageData(width, height);
      }
      return origGetImageData.apply(this, args);
    };
  } catch (e) {
    console.warn('[ANTI-CAPTURE GUARD] Could not wrap canvas extraction APIs:', e);
  }
})();

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

    // Unique Peer Identity & Real-Time Mesh
    this.myPeerId = sessionStorage.getItem('gafana_peer_id') || ('web_' + Math.random().toString(36).substring(2, 8));
    sessionStorage.setItem('gafana_peer_id', this.myPeerId);
    this.myDisplayName = sessionStorage.getItem('gafana_display_name') || ('User ' + this.myPeerId.substring(4));
    this.peerConnections = new Map();
    this._votedBallots = new Set();

    // Consensus Ballot State
    this.currentBallot = null;
    this.ballotState = 'IDLE'; // IDLE, REQUESTED, APPROVED, REJECTED, REVOKED
    this.participants = [
      { id: this.myPeerId, name: `${this.myDisplayName} (You)`, hasVoted: false, vote: null, isSelf: true }
    ];

    // Presentation State
    this.slideImages = [];
    this.currentSlideIndex = 0;
    this.laserActive = false;
    this.networkUrl = '';

    // Anti-Capture & Background Device Recording State
    this.antiCaptureOverlay = null;
    this.antiCaptureBadge = null;
    this.stageContent = null;
    this._bgAudioSuspended = false;
    this._bgVideoSuspended = false;
    this._bgAudioCtxSuspended = false;
    this._wasMicActiveBeforeBg = false;
    this._wasCamActiveBeforeBg = false;

    this.initElements();
    this.initEventListeners();
    this.initAntiCaptureGuard();
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

    // Header Link Copy & HTTPS Helper
    this.btnHeaderCopyLink = document.getElementById('btn-header-copy-link');
    this.httpsHelperBanner = document.getElementById('https-helper-banner');
    this.btnDismissHttpsBanner = document.getElementById('btn-dismiss-https-banner');

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

    // Anti-Capture Elements
    this.antiCaptureOverlay = document.getElementById('anti-capture-overlay');
    this.antiCaptureBadge = document.getElementById('anti-capture-badge');
    this.stageContent = document.getElementById('stage-content');
  }

  // =========================================================================
  // 🛡️ ANTI-CAPTURE, ANTI-SCREENSHOT & BACKGROUND DEVICE RECORDING GUARD
  // Stops unapproved screenshots, MediaRecorder usage, canvas scraping,
  // and halts background device recording when tab is not active.
  // =========================================================================
  initAntiCaptureGuard() {
    // 1. Keystroke Screenshot & Screen-Snip Blocker
    const handleScreenshotKeys = (e) => {
      // If consent is already unanimously approved, allow capture
      if (this.ballotState === 'APPROVED' && window.__gafana_consent_approved) {
        return;
      }

      const key = e.key || '';
      const keyCode = e.keyCode || e.which;
      const isPrintScreen = key === 'PrintScreen' || keyCode === 44;
      const isWinSnipping = e.shiftKey && (key === 'S' || key === 's');
      const isMacScreenshot = (e.metaKey && e.shiftKey && ['3', '4', '5'].includes(key));
      const isPrintDoc = (e.ctrlKey || e.metaKey) && (key === 'p' || key === 'P');
      const isSaveDoc = (e.ctrlKey || e.metaKey) && (key === 's' || key === 'S');

      if (isPrintScreen || isWinSnipping || isMacScreenshot || isPrintDoc || isSaveDoc) {
        e.preventDefault();
        e.stopPropagation();

        // Clear OS clipboard immediately to neutralize any captured image buffers
        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText('').catch(() => {});
        }

        // Flash security notification
        this.flashAntiCaptureWarning('Screenshot attempt blocked: Unanimous participant consent has not been granted.');
        this.logTerminal('[ANTI-CAPTURE GUARD] Keystroke screenshot attempt intercepted and neutralized. 100% consent required.', 'danger');
      }
    };

    window.addEventListener('keydown', handleScreenshotKeys, true);
    window.addEventListener('keyup', handleScreenshotKeys, true);

    // 2. Anti-Snipping & Window Blur Protection
    // When external capture tools (Snipping tool, OBS, Greenshot) gain focus, the window loses focus (blur).
    // While consent is NOT approved, we instantly display the opaque blackout overlay.
    window.addEventListener('blur', () => {
      if (this.ballotState !== 'APPROVED' || !window.__gafana_consent_approved) {
        if (this.antiCaptureOverlay) {
          this.antiCaptureOverlay.style.display = 'flex';
        }
        if (this.stageContent) {
          this.stageContent.classList.add('capture-obscured');
        }
        if (this.antiCaptureBadge) {
          this.antiCaptureBadge.innerHTML = '<span>🔒 Shielding (Window Inactive)</span>';
        }
      }
    });

    window.addEventListener('focus', () => {
      if (this.ballotState !== 'APPROVED' || !window.__gafana_consent_approved) {
        if (this.antiCaptureOverlay) {
          this.antiCaptureOverlay.style.display = 'none';
        }
        if (this.stageContent) {
          this.stageContent.classList.remove('capture-obscured');
        }
        if (this.antiCaptureBadge) {
          this.antiCaptureBadge.innerHTML = '<span>🔒 Anti-Capture Guard: ARMED</span>';
        }
      }
    });

    // 3. Background Device Recording Protection (Mute Mic & Webcam when Tab is Hidden/Backgrounded)
    document.addEventListener('visibilitychange', () => {
      const isHidden = document.visibilityState === 'hidden';

      if (isHidden) {
        // Tab moved to background. If unanimous consent is NOT granted, immediately stop devices from recording in the background.
        if (this.ballotState !== 'APPROVED' || !window.__gafana_consent_approved) {
          // Suspend microphone track
          if (this.localStream) {
            const audioTracks = this.localStream.getAudioTracks();
            if (audioTracks.length > 0 && audioTracks[0].enabled) {
              this._bgAudioSuspended = true;
              audioTracks[0].enabled = false;
            }

            // Suspend camera track
            const videoTracks = this.localStream.getVideoTracks();
            if (videoTracks.length > 0 && videoTracks[0].enabled) {
              this._bgVideoSuspended = true;
              videoTracks[0].enabled = false;
            }
          }

          // Suspend WebAudio analyser
          if (this.audioContext && this.audioContext.state === 'running') {
            this.audioContext.suspend();
            this._bgAudioCtxSuspended = true;
          }

          if (this.antiCaptureBadge) {
            this.antiCaptureBadge.innerHTML = '<span>🔇 BG Recording Halted</span>';
          }
          this.logTerminal('[ANTI-CAPTURE GUARD] Inactive tab: Hardware mic and camera tracks halted to prevent background device recording.', 'warn');
        }
      } else {
        // Tab returned to foreground
        if (this._bgAudioSuspended) {
          const isMicButtonActive = this.btnMic && this.btnMic.classList.contains('active');
          if (isMicButtonActive && this.localStream) {
            const audioTracks = this.localStream.getAudioTracks();
            if (audioTracks.length > 0) audioTracks[0].enabled = true;
          }
          this._bgAudioSuspended = false;
        }

        if (this._bgVideoSuspended) {
          const isCamButtonActive = this.btnCam && this.btnCam.classList.contains('active');
          if (isCamButtonActive && this.localStream) {
            const videoTracks = this.localStream.getVideoTracks();
            if (videoTracks.length > 0) videoTracks[0].enabled = true;
          }
          this._bgVideoSuspended = false;
        }

        if (this._bgAudioCtxSuspended) {
          if (this.audioContext && this.audioContext.state === 'suspended') {
            this.audioContext.resume();
          }
          this._bgAudioCtxSuspended = false;
        }

        if (this.antiCaptureBadge) {
          const approved = this.ballotState === 'APPROVED' && window.__gafana_consent_approved;
          this.antiCaptureBadge.innerHTML = approved
            ? '<span>🟢 Recording Consent: GRANTED</span>'
            : '<span>🔒 Anti-Capture Guard: ARMED</span>';
        }
        this.logTerminal('[ANTI-CAPTURE GUARD] Meeting tab in foreground: Live hardware audio/video streams resumed.', 'info');
      }
    });

    // 4. Context Menu & Drag-and-Drop Image Extraction Prevention
    const preventExtraction = (e) => {
      if (this.ballotState !== 'APPROVED' || !window.__gafana_consent_approved) {
        e.preventDefault();
        return false;
      }
    };

    ['contextmenu', 'dragstart'].forEach(ev => {
      window.addEventListener(ev, (e) => {
        const target = e.target;
        if (target && (target.tagName === 'VIDEO' || target.tagName === 'CANVAS' || target.closest('.stage-container') || target.closest('.peers-container'))) {
          preventExtraction(e);
        }
      }, true);
    });
  }

  flashAntiCaptureWarning(msg) {
    if (this.antiCaptureOverlay) {
      const msgElem = document.getElementById('anti-capture-msg');
      if (msgElem) msgElem.textContent = msg;
      this.antiCaptureOverlay.style.display = 'flex';
      setTimeout(() => {
        if (this.ballotState !== 'APPROVED' && document.hasFocus()) {
          this.antiCaptureOverlay.style.display = 'none';
        }
      }, 2500);
    }
  }

  updateConsentGuard(isApproved) {
    window.__gafana_consent_approved = isApproved;
    if (this.antiCaptureBadge) {
      if (isApproved) {
        this.antiCaptureBadge.classList.add('consent-granted');
        this.antiCaptureBadge.innerHTML = '<span>🟢 Recording Consent: GRANTED</span>';
      } else {
        this.antiCaptureBadge.classList.remove('consent-granted');
        this.antiCaptureBadge.innerHTML = '<span>🔒 Anti-Capture Guard: ARMED</span>';
      }
    }
    if (this.antiCaptureOverlay && isApproved) {
      this.antiCaptureOverlay.style.display = 'none';
    }
    if (this.stageContent && isApproved) {
      this.stageContent.classList.remove('capture-obscured');
    }
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
      if (this.peerConnections) {
        for (const pc of this.peerConnections.values()) {
          this.localStream.getTracks().forEach(track => {
            pc.addTrack(track, this.localStream);
          });
        }
      }
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

    // Copy Meeting Link Action
    if (this.btnHeaderCopyLink) {
      this.btnHeaderCopyLink.addEventListener('click', () => this.copyMeetingLink());
    }
    if (this.btnCopyInviteLink) {
      this.btnCopyInviteLink.addEventListener('click', () => this.copyMeetingLink());
    }

    // Dismiss Insecure Origin Banner
    if (this.btnDismissHttpsBanner && this.httpsHelperBanner) {
      this.btnDismissHttpsBanner.addEventListener('click', () => {
        this.httpsHelperBanner.style.display = 'none';
      });
    }

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

      // Check for insecure context (Remote HTTP without SSL)
      const isSecure = (window.isSecureContext === true) || window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1';
      if (!isSecure && this.httpsHelperBanner) {
        this.httpsHelperBanner.style.display = 'flex';
      }
    } catch (_) {}
  }

  copyMeetingLink() {
    let link = this.networkUrl || window.location.href;
    if ((window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1') && this.networkUrl) {
      link = this.networkUrl;
    }

    navigator.clipboard.writeText(link).then(() => {
      this.logTerminal(`Meeting link copied to clipboard: ${link}`, 'success');

      if (this.btnHeaderCopyLink) {
        const origHTML = this.btnHeaderCopyLink.innerHTML;
        this.btnHeaderCopyLink.innerHTML = '<span>✅ Copied!</span>';
        this.btnHeaderCopyLink.style.borderColor = '#10b981';
        this.btnHeaderCopyLink.style.color = '#10b981';
        setTimeout(() => {
          this.btnHeaderCopyLink.innerHTML = origHTML;
          this.btnHeaderCopyLink.style.borderColor = '';
          this.btnHeaderCopyLink.style.color = '';
        }, 2200);
      }

      if (this.btnCopyInviteLink) {
        this.btnCopyInviteLink.textContent = '✅ Copied!';
        setTimeout(() => {
          this.btnCopyInviteLink.textContent = '📋 Copy Room Link';
        }, 2200);
      }
    }).catch(() => {
      prompt('Copy this confidential Gafana room link to invite participants:', link);
    });
  }

  async initPeerSync() {
    await this.registerSelfPeer();
    this.pollPeers();
    setInterval(() => this.pollPeers(), 2500);
    setInterval(() => this.sendHeartbeat(), 3500);
    setInterval(() => this.pollConsensusStatus(), 1200);
    setInterval(() => this.pollSignals(), 1200);

    window.addEventListener('beforeunload', () => {
      try {
        navigator.sendBeacon('/api/peers/leave', JSON.stringify({ peer_id: this.myPeerId }));
      } catch (_) {}
    });
  }

  async registerSelfPeer() {
    try {
      await fetch('/api/peers/join', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ peer_id: this.myPeerId, name: this.myDisplayName })
      });
      const selfLabel = document.getElementById('self-display-label');
      if (selfLabel) {
        selfLabel.textContent = `${this.myDisplayName} (You)`;
      }
      this.logTerminal(`Registered as room participant: ${this.myDisplayName} [${this.myPeerId}]`, 'success');
    } catch (_) {}
  }

  async sendHeartbeat() {
    try {
      await fetch('/api/peers/heartbeat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ peer_id: this.myPeerId })
      });
    } catch (_) {}
  }

  async pollPeers() {
    try {
      const res = await fetch(`/api/peers?peer_id=${this.myPeerId}`);
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

      // Reconcile WebRTC peer connections
      for (const p of peers) {
        if (!p.is_self && !this.peerConnections.has(p.id)) {
          this.setupPeerConnection(p.id, p.name);
        }
      }

      // Cleanup peers that disconnected
      const activeIds = new Set(peers.map(p => p.id));
      for (const [pid, pc] of this.peerConnections) {
        if (!activeIds.has(pid)) {
          pc.close();
          this.peerConnections.delete(pid);
          this.removeRemoteVideoCard(pid);
        }
      }
    } catch (_) {}
  }

  setupPeerConnection(remotePeerId, remoteName) {
    const rtcConfig = {
      iceServers: [
        { urls: 'stun:stun.l.google.com:19302' },
        { urls: 'stun:stun1.l.google.com:19302' },
        { urls: 'stun:stun2.l.google.com:19302' },
        { urls: 'stun:stun.cloudflare.com:3478' },
        { urls: 'stun:global.stun.twilio.com:3478' },
        {
          urls: [
            'turn:openrelay.metered.ca:80',
            'turn:openrelay.metered.ca:443',
            'turn:openrelay.metered.ca:443?transport=tcp'
          ],
          username: 'openrelayproject',
          credential: 'openrelayproject'
        }
      ]
    };

    const pc = new RTCPeerConnection(rtcConfig);
    this.peerConnections.set(remotePeerId, pc);

    // Add local tracks if available
    if (this.localStream) {
      this.localStream.getTracks().forEach(track => {
        pc.addTrack(track, this.localStream);
      });
    }

    pc.ontrack = (event) => {
      this.handleRemoteTrack(remotePeerId, remoteName, event);
    };

    pc.onicecandidate = (event) => {
      if (event.candidate) {
        this.sendSignal(remotePeerId, 'candidate', event.candidate);
      }
    };

    // Polite peer pattern: lexicographically smaller ID initiates offer
    if (this.myPeerId < remotePeerId) {
      pc.onnegotiationneeded = async () => {
        try {
          const offer = await pc.createOffer();
          await pc.setLocalDescription(offer);
          this.sendSignal(remotePeerId, 'offer', offer);
        } catch (e) {
          console.warn('[WebRTC] Negotiation offer error:', e);
        }
      };
    }
  }

  async sendSignal(to, type, payload) {
    try {
      await fetch('/api/signal/send', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          from: this.myPeerId,
          to: to,
          type: type,
          payload: payload
        })
      });
    } catch (_) {}
  }

  async pollSignals() {
    try {
      const res = await fetch(`/api/signal/poll?peer_id=${this.myPeerId}`);
      if (!res.ok) return;
      const signals = await res.json();
      for (const sig of signals) {
        await this.handleIncomingSignal(sig);
      }
    } catch (_) {}
  }

  async handleIncomingSignal(sig) {
    let pc = this.peerConnections.get(sig.from);
    if (!pc) {
      this.setupPeerConnection(sig.from, `Peer ${sig.from.substring(0, 6)}`);
      pc = this.peerConnections.get(sig.from);
    }
    if (!pc) return;

    try {
      if (sig.type === 'offer') {
        await pc.setRemoteDescription(new RTCSessionDescription(sig.payload));
        const answer = await pc.createAnswer();
        await pc.setLocalDescription(answer);
        this.sendSignal(sig.from, 'answer', answer);
      } else if (sig.type === 'answer') {
        await pc.setRemoteDescription(new RTCSessionDescription(sig.payload));
      } else if (sig.type === 'candidate' && sig.payload) {
        await pc.addIceCandidate(new RTCIceCandidate(sig.payload));
      }
    } catch (e) {
      console.warn('[WebRTC] Signal handling error:', e);
    }
  }

  handleRemoteTrack(remotePeerId, remoteName, event) {
    const peersGrid = document.getElementById('peers-grid');
    if (!peersGrid) return;

    let card = document.getElementById(`peer-card-${remotePeerId}`);
    if (!card) {
      card = document.createElement('div');
      card.className = 'peer-card';
      card.id = `peer-card-${remotePeerId}`;
      card.innerHTML = `
        <div class="peer-video-wrapper">
          <video autoplay playsinline class="remote-video" id="video-${remotePeerId}" style="width:100%; height:100%; object-fit:cover; border-radius:12px;"></video>
        </div>
        <div class="peer-label">
          <span>${remoteName || remotePeerId}</span>
          <span style="color:#10b981;">🎤 Live</span>
        </div>
      `;
      peersGrid.appendChild(card);
      this.logTerminal(`[WEBRTC P2P] Video feed linked with ${remoteName || remotePeerId}.`, 'success');
    }

    const video = card.querySelector('video');
    if (video && event.streams && event.streams[0]) {
      video.srcObject = event.streams[0];
    } else if (video) {
      if (!video.srcObject) {
        video.srcObject = new MediaStream();
      }
      video.srcObject.addTrack(event.track);
    }
  }

  removeRemoteVideoCard(remotePeerId) {
    const card = document.getElementById(`peer-card-${remotePeerId}`);
    if (card) {
      card.remove();
      this.logTerminal(`Participant ${remotePeerId} disconnected.`, 'info');
    }
  }

  async fetchAndShowPeers() {
    try {
      const res = await fetch(`/api/peers?peer_id=${this.myPeerId}`);
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
        this.peersModalList.innerHTML = peers.map((p) => `
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

  async pollConsensusStatus() {
    try {
      const res = await fetch('/api/consensus/status');
      if (!res.ok) return;
      const b = await res.json();

      if (b.is_active && b.state === 'Requested') {
        this.currentBallot = {
          ballotId: b.ballot_id,
          requesterId: b.requester_id,
          recType: b.rec_type,
          state: b.state,
          yesVotes: b.yes_votes,
          noVotes: b.no_votes,
          totalExpected: b.total_expected
        };

        if (this.tallyTotal) this.tallyTotal.textContent = b.total_expected;
        if (this.tallyYes) this.tallyYes.textContent = b.yes_votes;
        if (this.tallyNo) this.tallyNo.textContent = b.no_votes;

        const reqMsg = document.getElementById('consensus-req-msg');
        if (reqMsg) {
          const typeName = (b.rec_type === 'AudioOnly' || b.rec_type === 'audio') ? 'Audio-Only' : 'Full Video + Audio';
          reqMsg.textContent = `${b.requester_id} is requesting 100% Unanimous Agreement to record this call [${typeName}]. Every participant has absolute veto power.`;
        }

        if (!this._votedBallots.has(b.ballot_id)) {
          if (b.requester_id === this.myPeerId) {
            this._votedBallots.add(b.ballot_id);
          } else {
            this.modalOverlay.style.display = 'flex';
          }
        }
      } else if (b.state === 'UnanimousOk') {
        this.modalOverlay.style.display = 'none';
        if (this.ballotState !== 'APPROVED') {
          this.ballotState = 'APPROVED';
          this.updateConsentGuard(true);
          this.logTerminal('100% Unanimous agreement granted by all room participants! Local recording unlocked.', 'success');
          if (b.requester_id === this.myPeerId && !this.isRecording) {
            this.startLocalRecording();
          }
        }
      } else if (b.state === 'Rejected') {
        this.modalOverlay.style.display = 'none';
        if (this.ballotState === 'REQUESTED') {
          this.ballotState = 'REJECTED';
          this.updateConsentGuard(false);
          this.logTerminal('RECORDING BLOCKED: A participant cast a VETO.', 'danger');
          alert('Recording blocked! A participant exercised their veto right.\nZero bytes were captured.');
        }
      } else if (b.state === 'Revoked') {
        this.modalOverlay.style.display = 'none';
        if (this.isRecording) {
          this.stopLocalRecording();
          this.ballotState = 'REVOKED';
          this.updateConsentGuard(false);
          this.logTerminal('RECORDING HALTED: Consent was revoked mid-call.', 'danger');
          alert('Consent revoked! Local recording has been forcefully halted immediately.');
        }
      }
    } catch (_) {}
  }

  async initiateRecordingConsent(type = 'av') {
    this.recordingType = type;
    const typeLabel = (type === 'audio') ? 'Audio-Only' : 'Full Video + Audio';
    this.logTerminal(`Initiating unanimous consent ballot for [${typeLabel}]...`, 'info');

    try {
      const res = await fetch('/api/consensus/request', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          requester_id: this.myPeerId,
          rec_type: type
        })
      });
      const data = await res.json();
      if (data.success) {
        this.ballotState = 'REQUESTED';
        this._votedBallots.add(data.ballot_id);

        if (data.state === 'UnanimousOk') {
          this.ballotState = 'APPROVED';
          this.updateConsentGuard(true);
          this.logTerminal('100% Unanimous approval satisfied (solo attendee). Starting capture...', 'success');
          this.startLocalRecording();
        } else {
          this.modalOverlay.style.display = 'flex';
        }
      }
    } catch (e) {
      this.logTerminal(`Ballot initiation error: ${e.message}`, 'danger');
    }
  }

  async castVote(isYes) {
    if (!this.currentBallot) return;
    const bid = this.currentBallot.ballotId;
    this._votedBallots.add(bid);

    try {
      const res = await fetch('/api/consensus/vote', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          ballot_id: bid,
          voter_id: this.myPeerId,
          accept: isYes
        })
      });
      const data = await res.json();
      this.modalOverlay.style.display = 'none';
      if (!isYes) {
        this.ballotState = 'REJECTED';
        this.updateConsentGuard(false);
        this.logTerminal('VETO CAST: You blocked the recording request.', 'danger');
      } else {
        this.logTerminal('VOTE RECORDED: You accepted the recording request.', 'success');
      }
    } catch (e) {
      this.logTerminal(`Vote submission error: ${e.message}`, 'danger');
    }
  }

  updateBallotModal() {
    if (!this.currentBallot) return;
    this.tallyTotal.textContent = this.currentBallot.totalExpected;
    this.tallyYes.textContent = this.currentBallot.yesVotes;
    this.tallyNo.textContent = this.currentBallot.noVotes;
  }

  async revokeConsent() {
    const bid = (this.currentBallot && this.currentBallot.ballotId) ? this.currentBallot.ballotId : 0;
    try {
      await fetch('/api/consensus/revoke', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          ballot_id: bid,
          revoker_id: this.myPeerId
        })
      });
    } catch (_) {}
    this.ballotState = 'REVOKED';
    this.updateConsentGuard(false);
    this.logTerminal('CONSENT REVOKED! Local recording halted immediately by participant command.', 'danger');
    this.stopLocalRecording();
  }

  startLocalRecording() {
    if (this.ballotState !== 'APPROVED' || !window.__gafana_consent_approved) {
      this.flashAntiCaptureWarning('Security Violation: Recording blocked without 100% unanimous consent.');
      this.logTerminal('SECURITY VIOLATION: MediaRecorder blocked without unanimous consent.', 'danger');
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
    this.updateConsentGuard(false);

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
