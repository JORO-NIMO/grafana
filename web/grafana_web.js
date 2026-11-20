/**
 * Grafana Production Web Client Engine
 *
 * Implements:
 *  - Zero-Trust Browser Extension Shield
 *  - Real WebRTC MediaStream acquisition (getUserMedia & getDisplayMedia)
 *  - Screen Sharing with WebRTC track replacement & remote stage spotlight
 *  - Optional Face Cam picture-in-picture overlay with custom drag positioning
 *  - Participant admission control (Auto-Admit toggle and manual confirm/deny)
 *  - User profile customization (display name and custom profile picture)
 *  - Mid-call recording joining without disruption or re-balloting
 *  - Clean UI with zero emojis and short, clear button descriptions
 *  - Client-side MediaRecorder with zero cloud retention
 */

// =========================================================================
// ZERO-TRUST BROWSER EXTENSION SHIELD
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

    if ((el.classList && el.classList.contains('Grafana-app')) || tag.startsWith('Grafana')) {
      return false;
    }

    if (tag.includes('-') && !tag.startsWith('Grafana-')) {
      return true;
    }

    const src = el.getAttribute('src') || '';
    const href = el.getAttribute('href') || '';
    if (src.includes('-extension://') || href.includes('-extension://')) {
      return true;
    }

    const id = (el.id || '').toLowerCase();
    const className = (typeof el.className === 'string' ? el.className : '').toLowerCase();

    for (const pat of extensionPatterns) {
      if (id.includes(pat) && !id.includes('Grafana')) return true;
      if (className.includes(pat) && !className.includes('Grafana')) return true;
    }

    if (el.hasAttribute('data-extension-element') ||
      el.hasAttribute('data-loom') ||
      el.hasAttribute('data-lastpass-root') ||
      el.hasAttribute('data-bitwarden-watching') ||
      el.hasAttribute('data-injected') ||
      el.hasAttribute('data-grammarly-shadow-root')) {
      return true;
    }

    if (el.style && el.style.zIndex && parseInt(el.style.zIndex) > 100000 && !className.includes('Grafana')) {
      return true;
    }

    return false;
  }

  function purgeNode(node) {
    if (isExtensionElement(node)) {
      blockedCount++;
      try {
        node.style.setProperty('display', 'none', 'important');
        node.style.setProperty('visibility', 'hidden', 'important');
        node.remove();
      } catch (e) { }
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
      badge.innerHTML = `<span>Shield: ${count} Blocked</span>`;
      badge.style.display = 'inline-flex';
    }
  }

  try {
    const origAttachShadow = Element.prototype.attachShadow;
    Element.prototype.attachShadow = function (init) {
      if (isExtensionElement(this)) {
        this.remove();
        return document.createDocumentFragment();
      }
      return origAttachShadow.call(this, init);
    };
  } catch (e) { }

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

  window.addEventListener('DOMContentLoaded', () => {
    document.querySelectorAll('*').forEach(purgeNode);
  });
  window.addEventListener('load', () => {
    document.querySelectorAll('*').forEach(purgeNode);
  });

  try {
    if (navigator.mediaDevices && navigator.mediaDevices.getUserMedia) {
      Object.freeze(navigator.mediaDevices.getUserMedia);
    }
  } catch (e) { }

  window.__Grafana_shield_active = true;
})();

// =========================================================================
// ANTI-CAPTURE & MEDIA PROTECTION HOOKS
// =========================================================================
window.__Grafana_consent_approved = false;

(function installMediaRecorderGuard() {
  try {
    const OrigMediaRecorder = window.MediaRecorder;
    if (OrigMediaRecorder) {
      window.__OrigMediaRecorder = OrigMediaRecorder;
      window.MediaRecorder = function (stream, options) {
        if (!window.__Grafana_consent_approved) {
          throw new DOMException(
            'Recording is prohibited: No unanimous consent granted.',
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
  } catch (e) { }
})();

(function installCanvasScrapeNeutralizer() {
  try {
    const origToDataURL = HTMLCanvasElement.prototype.toDataURL;
    const origToBlob = HTMLCanvasElement.prototype.toBlob;
    const origGetImageData = CanvasRenderingContext2D.prototype.getImageData;

    HTMLCanvasElement.prototype.toDataURL = function (...args) {
      if (!window.__Grafana_consent_approved) {
        return 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';
      }
      return origToDataURL.apply(this, args);
    };

    HTMLCanvasElement.prototype.toBlob = function (callback, ...args) {
      if (!window.__Grafana_consent_approved) {
        if (typeof callback === 'function') {
          callback(new Blob([], { type: 'image/png' }));
        }
        return;
      }
      return origToBlob.call(this, callback, ...args);
    };

    CanvasRenderingContext2D.prototype.getImageData = function (...args) {
      if (!window.__Grafana_consent_approved) {
        const width = args[2] || 1;
        const height = args[3] || 1;
        return this.createImageData(width, height);
      }
      return origGetImageData.apply(this, args);
    };
  } catch (e) { }
})();

// =========================================================================
// MAIN Grafana WEB CLIENT
// =========================================================================
class GrafanaWebClient {
  constructor() {
    this.localStream = null;
    this.screenStream = null;
    this.isScreenSharing = false;
    this.activeScreenTrack = null;
    this.audioContext = null;
    this.analyser = null;
    this.mediaRecorder = null;
    this.recordedChunks = [];
    this.isRecording = false;
    this.recordingType = 'av';
    this.recordStartTime = 0;
    this.recordTimerInterval = null;

    // Face Cam overlay settings (screen sharing camera PiP)
    this.faceCamActive = false;
    this.faceCamPos = { x: 30, y: 30, width: 220, height: 140 };
    this.compositeAnimId = null;

    // Identity, profile, and admission state
    this.myPeerId = sessionStorage.getItem('Grafana_peer_id') || ('web_' + Math.random().toString(36).substring(2, 8));
    sessionStorage.setItem('Grafana_peer_id', this.myPeerId);
    this.myDisplayName = localStorage.getItem('Grafana_display_name') || sessionStorage.getItem('Grafana_display_name') || ('User ' + this.myPeerId.substring(4));
    this.myAvatar = localStorage.getItem('Grafana_avatar_data') || '';
    this.userStatus = 'admitted';
    this.autoAdmit = true;
    this.pendingAdmitPeers = new Set();

    this.peerConnections = new Map();
    this.remotePeerStates = new Map(); // peerId -> { screen_sharing: bool, name: string, avatar: string }
    this._votedBallots = new Set();

    // Consensus state
    this.currentBallot = null;
    this.ballotState = 'IDLE';
    this.participants = [
      { id: this.myPeerId, name: `${this.myDisplayName} (You)`, hasVoted: false, vote: null, isSelf: true }
    ];

    // Media & video elements
    this.localVideo = document.getElementById('local-video');
    this.screenVideo = document.getElementById('screen-video');
    this.faceCamOverlay = document.getElementById('facecam-overlay');
    this.faceCamVideo = document.getElementById('facecam-video');
    this.faceCamDragHandle = document.getElementById('facecam-drag-handle');
    this.btnCloseFaceCam = document.getElementById('btn-close-facecam');
    this.compositeCanvas = document.getElementById('composite-canvas');
    this.compositeCtx = this.compositeCanvas ? this.compositeCanvas.getContext('2d') : null;
    this.videoPlaceholder = document.getElementById('video-placeholder');
    this.placeholderAvatar = document.getElementById('placeholder-avatar');

    // Control dock buttons
    this.btnMic = document.getElementById('btn-mic');
    this.btnCam = document.getElementById('btn-cam');
    this.btnScreen = document.getElementById('btn-screenshare');
    this.btnFaceCam = document.getElementById('btn-facecam');
    this.btnRecord = document.getElementById('btn-record');
    this.btnLeave = document.getElementById('btn-leave');

    // Draggable facecam overlay
    this.faceCamDragging = false;
    this.faceCamOffsetX = 0;
    this.faceCamOffsetY = 0;

    // Anti-capture background tracking
    this._bgAudioSuspended = false;
    this._bgVideoSuspended = false;
    this._bgAudioCtxSuspended = false;
    this.networkUrl = '';

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

    // Header buttons
    this.btnHeaderCopyLink = document.getElementById('btn-header-copy-link');
    this.btnHeaderAutoAdmit = document.getElementById('btn-header-autoadmit');
    this.autoAdmitLabel = document.getElementById('autoadmit-label');
    this.btnHeaderProfile = document.getElementById('btn-header-profile');
    this.btnDockProfile = document.getElementById('btn-dock-profile');
    this.httpsHelperBanner = document.getElementById('https-helper-banner');
    this.btnDismissHttpsBanner = document.getElementById('btn-dismiss-https-banner');

    // Profile modal
    this.profileModal = document.getElementById('profile-modal');
    this.btnCloseProfileModal = document.getElementById('btn-close-profile-modal');
    this.profileNameInput = document.getElementById('profile-name-input');
    this.profileAvatarInput = document.getElementById('profile-avatar-input');
    this.profilePreviewContainer = document.getElementById('profile-preview-container');
    this.profileInitialsPreview = document.getElementById('profile-initials-preview');
    this.btnSaveProfile = document.getElementById('btn-save-profile');
    this.btnClearAvatar = document.getElementById('btn-clear-avatar');

    // Admission & Waiting elements
    this.waitingModal = document.getElementById('waiting-modal');
    this.admissionToast = document.getElementById('admission-toast');
    this.admissionPeerName = document.getElementById('admission-peer-name');
    this.btnAdmitAccept = document.getElementById('btn-admit-accept');
    this.btnAdmitDeny = document.getElementById('btn-admit-deny');

    // Command Center
    this.cmdModal = document.getElementById('command-center-modal');
    this.btnHeaderMenu = document.getElementById('btn-header-menu');
    this.btnDockMenu = document.getElementById('btn-dock-menu');
    this.btnCloseCmdMenu = document.getElementById('btn-close-cmd-menu');
    this.cmdTermOutput = document.getElementById('cmd-term-output');

    this.cmdBtnPeers = document.getElementById('cmd-btn-peers');
    this.cmdBtnRecordA = document.getElementById('cmd-btn-record-a');
    this.cmdBtnRecordV = document.getElementById('cmd-btn-record-v');
    this.cmdBtnAccept = document.getElementById('cmd-btn-accept');
    this.cmdBtnDeny = document.getElementById('cmd-btn-deny');
    this.cmdBtnStartrec = document.getElementById('cmd-btn-startrec');
    this.cmdBtnStoprec = document.getElementById('cmd-btn-stoprec');
    this.cmdBtnRevoke = document.getElementById('cmd-btn-revoke');
    this.cmdBtnScreen = document.getElementById('cmd-btn-screen');
    this.cmdBtnFaceCam = document.getElementById('cmd-btn-facecam');
    this.cmdBtnPresentNext = document.getElementById('cmd-btn-present-next');
    this.cmdBtnStats = document.getElementById('cmd-btn-stats');
    this.cmdBtnOpen = document.getElementById('cmd-btn-open');
    this.cmdBtnQuit = document.getElementById('cmd-btn-quit');
    this.cmdBtnInvite = document.getElementById('cmd-btn-invite');
    this.cmdBtnToggleAutoAdmit = document.getElementById('cmd-btn-toggle-autoadmit');

    // SMTP elements
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

    // Peers modal
    this.peersModal = document.getElementById('peers-modal');
    this.btnClosePeersModal = document.getElementById('btn-close-peers-modal');
    this.peersModalList = document.getElementById('peers-modal-list');
    this.btnCopyInviteLink = document.getElementById('btn-copy-invite-link');
    this.btnRefreshPeers = document.getElementById('btn-refresh-peers');

    // Anti-Capture
    this.antiCaptureOverlay = document.getElementById('anti-capture-overlay');
    this.antiCaptureBadge = document.getElementById('anti-capture-badge');
    this.stageContent = document.getElementById('stage-content');

    // Landing Page Elements
    this.landingDisplayName = document.getElementById('landing-display-name');
    this.landingRoomName = document.getElementById('landing-room-name');
    this.btnLandingEnter = document.getElementById('btn-landing-enter');
    this.btnLandingInvite = document.getElementById('btn-landing-invite') || document.getElementById('btn-landing-smtp');
  }

  // =========================================================================
  // PROFILE & CUSTOM AVATAR MANAGEMENT
  // =========================================================================
  initProfile() {
    this.updateSelfDisplay();

    if (this.landingDisplayName) {
      this.landingDisplayName.value = this.myDisplayName;
      this.landingDisplayName.addEventListener('input', () => {
        const val = this.landingDisplayName.value.trim();
        if (val) {
          this.myDisplayName = val;
          localStorage.setItem('Grafana_display_name', val);
          this.updateSelfDisplay();
          this.broadcastProfileUpdate();
        }
      });
    }

    if (this.btnLandingEnter) {
      this.btnLandingEnter.addEventListener('click', () => {
        this.btnLandingEnter.classList.add('btn-loading');
        setTimeout(() => {
          if (this.btnLandingEnter) this.btnLandingEnter.classList.remove('btn-loading');
        }, 600);
        if (this.landingDisplayName) {
          const val = this.landingDisplayName.value.trim();
          if (val) {
            this.myDisplayName = val;
            localStorage.setItem('Grafana_display_name', val);
            this.updateSelfDisplay();
            this.broadcastProfileUpdate();
          }
        }
        const stage = document.getElementById('stage-section');
        if (stage) {
          stage.scrollIntoView({ behavior: 'smooth', block: 'start' });
        }
      });
    }

    if (this.btnLandingInvite) {
      this.btnLandingInvite.addEventListener('click', () => {
        this.openSmtpModal();
      });
    }

    const openProfile = () => {
      if (!this.profileModal) return;
      this.profileNameInput.value = this.myDisplayName;
      this.renderProfilePreview(this.myAvatar);
      this.profileModal.style.display = 'flex';
    };

    if (this.btnHeaderProfile) this.btnHeaderProfile.addEventListener('click', openProfile);
    if (this.btnDockProfile) this.btnDockProfile.addEventListener('click', openProfile);
    if (this.btnCloseProfileModal) this.btnCloseProfileModal.addEventListener('click', () => {
      this.profileModal.style.display = 'none';
    });

    if (this.profileAvatarInput) {
      this.profileAvatarInput.addEventListener('change', (e) => {
        const file = e.target.files[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = (ev) => {
          this._tempAvatar = ev.target.result;
          this.renderProfilePreview(this._tempAvatar);
        };
        reader.readAsDataURL(file);
      });
    }

    if (this.btnClearAvatar) {
      this.btnClearAvatar.addEventListener('click', () => {
        this._tempAvatar = '';
        this.renderProfilePreview('');
      });
    }

    if (this.btnSaveProfile) {
      this.btnSaveProfile.addEventListener('click', async () => {
        const newName = (this.profileNameInput.value || '').trim();
        if (newName) {
          this.myDisplayName = newName;
          localStorage.setItem('Grafana_display_name', newName);
          sessionStorage.setItem('Grafana_display_name', newName);
        }
        if (this._tempAvatar !== undefined) {
          this.myAvatar = this._tempAvatar;
          localStorage.setItem('Grafana_avatar_data', this.myAvatar);
        }

        this.updateSelfDisplay();
        this.profileModal.style.display = 'none';

        // Push update to server
        try {
          await fetch('/api/peers/profile', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              peer_id: this.myPeerId,
              name: this.myDisplayName,
              avatar: this.myAvatar
            })
          });
        } catch (_) { }

        this.logTerminal(`Profile updated: ${this.myDisplayName}`, 'success');
      });
    }
  }

  renderProfilePreview(avatarUrl) {
    if (!this.profilePreviewContainer) return;
    if (avatarUrl) {
      this.profilePreviewContainer.innerHTML = `<img src="${avatarUrl}" alt="Avatar">`;
    } else {
      const init = (this.myDisplayName || 'G').charAt(0).toUpperCase();
      this.profilePreviewContainer.innerHTML = `<span>${init}</span>`;
    }
  }

  updateSelfDisplay() {
    const selfLabel = document.getElementById('self-display-label');
    if (selfLabel) {
      selfLabel.textContent = `${this.myDisplayName} (You)`;
    }

    const preview = document.getElementById('self-avatar-preview');
    if (preview) {
      if (this.myAvatar) {
        preview.innerHTML = `<img src="${this.myAvatar}" class="peer-avatar-img" alt="Avatar">`;
      } else {
        const init = (this.myDisplayName || 'U').charAt(0).toUpperCase();
        preview.textContent = init;
      }
    }

    if (this.placeholderAvatar) {
      if (this.myAvatar) {
        this.placeholderAvatar.innerHTML = `<img src="${this.myAvatar}" class="peer-avatar-img" alt="Avatar">`;
      } else {
        this.placeholderAvatar.textContent = (this.myDisplayName || 'G').charAt(0).toUpperCase();
      }
    }
  }

  // =========================================================================
  // DRAGGABLE FACE CAM OVERLAY (SCREEN SHARING PiP)
  // =========================================================================
  initFaceCamDrag() {
    if (!this.faceCamOverlay || !this.faceCamDragHandle) return;

    let isDragging = false;
    let startX = 0;
    let startY = 0;
    let initialLeft = 0;
    let initialTop = 0;

    const onPointerDown = (e) => {
      isDragging = true;
      startX = e.clientX;
      startY = e.clientY;
      const rect = this.faceCamOverlay.getBoundingClientRect();
      const parentRect = this.stageContent ? this.stageContent.getBoundingClientRect() : { left: 0, top: 0 };
      initialLeft = rect.left - parentRect.left;
      initialTop = rect.top - parentRect.top;

      if (e.target && e.target.setPointerCapture && e.pointerId !== undefined) {
        try { e.target.setPointerCapture(e.pointerId); } catch (_) { }
      }
      e.preventDefault();
    };

    const onPointerMove = (e) => {
      if (!isDragging) return;
      const dx = e.clientX - startX;
      const dy = e.clientY - startY;

      let newLeft = initialLeft + dx;
      let newTop = initialTop + dy;

      const parentWidth = this.stageContent ? this.stageContent.clientWidth : window.innerWidth;
      const parentHeight = this.stageContent ? this.stageContent.clientHeight : window.innerHeight;
      const boxWidth = this.faceCamOverlay.offsetWidth || 220;
      const boxHeight = this.faceCamOverlay.offsetHeight || 140;

      newLeft = Math.max(8, Math.min(newLeft, parentWidth - boxWidth - 8));
      newTop = Math.max(8, Math.min(newTop, parentHeight - boxHeight - 8));

      this.faceCamOverlay.style.left = `${newLeft}px`;
      this.faceCamOverlay.style.top = `${newTop}px`;
      this.faceCamOverlay.style.right = 'auto';
      this.faceCamOverlay.style.bottom = 'auto';

      this.faceCamPos.x = newLeft;
      this.faceCamPos.y = newTop;
      this.faceCamPos.width = boxWidth;
      this.faceCamPos.height = boxHeight;
    };

    const onPointerUp = (e) => {
      if (!isDragging) return;
      isDragging = false;
      if (e.target && e.target.releasePointerCapture && e.pointerId !== undefined) {
        try { e.target.releasePointerCapture(e.pointerId); } catch (_) { }
      }
    };

    this.faceCamDragHandle.addEventListener('pointerdown', onPointerDown);
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', onPointerUp);
    window.addEventListener('pointercancel', onPointerUp);

    // Backward-compat for legacy browser touch events
    this.faceCamDragHandle.addEventListener('touchstart', (e) => {
      if (e.touches && e.touches.length > 0) {
        onPointerDown(e.touches[0]);
      }
    }, { passive: false });
    window.addEventListener('touchmove', (e) => {
      if (e.touches && e.touches.length > 0) {
        onPointerMove(e.touches[0]);
      }
    }, { passive: false });
    window.addEventListener('touchend', onPointerUp);

    if (this.btnCloseFaceCam) {
      this.btnCloseFaceCam.addEventListener('click', () => {
        this.toggleFaceCam(false);
      });
    }
  }

    if (this.btnFaceCam) {
      this.btnFaceCam.addEventListener('click', () => {
        this.toggleFaceCam(!this.faceCamActive);
      });
    }
  }

  toggleFaceCam(active) {
    this.faceCamActive = active;
    if (this.btnFaceCam) {
      this.btnFaceCam.classList.toggle('active', active);
      const text = this.btnFaceCam.querySelector('.btn-text');
      if (text) text.textContent = active ? 'Face Cam: On' : 'Face Cam: Off';
    }

    if (!active) {
      if (this.faceCamOverlay) this.faceCamOverlay.style.display = 'none';
      if (this.compositeAnimId) {
        cancelAnimationFrame(this.compositeAnimId);
        this.compositeAnimId = null;
      }
      // Revert to direct screen stream track
      if (this.isScreenSharing && this.screenStream) {
        const screenTrack = this.screenStream.getVideoTracks()[0];
        if (screenTrack) {
          this.activeScreenTrack = screenTrack;
          this.broadcastTrackToPeers(screenTrack);
        }
      }
      this.logTerminal('Face Cam overlay disabled.', 'info');
    } else {
      if (this.faceCamOverlay) this.faceCamOverlay.style.display = 'flex';
      if (this.faceCamVideo && this.localStream) {
        this.faceCamVideo.srcObject = this.localStream;
      }
      this.startCompositeScreenLoop();
      this.logTerminal('Face Cam overlay active. Drag to reposition.', 'success');
    }
  }

  startCompositeScreenLoop() {
    if (!this.compositeCanvas || !this.screenVideo || !this.faceCamVideo) return;
    const canvas = this.compositeCanvas;
    const ctx = this.compositeCtx;

    const render = () => {
      if (!this.isScreenSharing || !this.faceCamActive) return;

      if (this.screenVideo.readyState >= 2) {
        ctx.drawImage(this.screenVideo, 0, 0, canvas.width, canvas.height);
      } else {
        ctx.fillStyle = '#0a0d14';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
      }

      // Draw custom-positioned face camera frame
      if (this.faceCamVideo.readyState >= 2) {
        const stageW = this.stageContent ? this.stageContent.clientWidth : canvas.width;
        const stageH = this.stageContent ? this.stageContent.clientHeight : canvas.height;

        const scaleX = canvas.width / (stageW || 1);
        const scaleY = canvas.height / (stageH || 1);

        const x = (this.faceCamPos.x || 30) * scaleX;
        const y = (this.faceCamPos.y || 30) * scaleY;
        const w = (this.faceCamPos.width || 220) * scaleX;
        const h = (this.faceCamPos.height || 140) * scaleY;

        ctx.save();
        ctx.strokeStyle = '#00f2fe';
        ctx.lineWidth = 4;
        ctx.strokeRect(x, y, w, h);
        ctx.drawImage(this.faceCamVideo, x, y, w, h);
        ctx.restore();
      }

      this.compositeAnimId = requestAnimationFrame(render);
    };

    render();

    // Stream composed track to remote peers
    if (!this.compositeStream) {
      this.compositeStream = canvas.captureStream(30);
    }
    const compTrack = this.compositeStream.getVideoTracks()[0];
    if (compTrack) {
      this.activeScreenTrack = compTrack;
      this.broadcastTrackToPeers(compTrack);
    }
  }

  broadcastTrackToPeers(track) {
    if (!track) return;
    for (const pc of this.peerConnections.values()) {
      const senders = pc.getSenders();
      const videoSender = senders.find(s => s.track && s.track.kind === 'video');
      if (videoSender) {
        videoSender.replaceTrack(track).catch(e => console.warn('[WebRTC] replaceTrack error:', e));
      } else if (this.localStream) {
        pc.addTrack(track, this.localStream);
      }
    }
  }

  // =========================================================================
  // ANTI-CAPTURE & BACKGROUND RECORDING GUARD
  // =========================================================================
  initAntiCaptureGuard() {
    const handleScreenshotKeys = (e) => {
      if (this.ballotState === 'APPROVED' && window.__Grafana_consent_approved) return;

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

        if (navigator.clipboard && navigator.clipboard.writeText) {
          navigator.clipboard.writeText('').catch(() => { });
        }

        this.flashAntiCaptureWarning('Screenshot blocked: Unanimous consent required.');
        this.logTerminal('[GUARD] Screenshot key intercepted and cleared.', 'danger');
      }
    };

    window.addEventListener('keydown', handleScreenshotKeys, true);
    window.addEventListener('keyup', handleScreenshotKeys, true);

    window.addEventListener('blur', () => {
      if (this.ballotState !== 'APPROVED' || !window.__Grafana_consent_approved) {
        if (this.antiCaptureOverlay) this.antiCaptureOverlay.style.display = 'flex';
        if (this.stageContent) this.stageContent.classList.add('capture-obscured');
        if (this.antiCaptureBadge) this.antiCaptureBadge.innerHTML = '<span>Guard: Inactive</span>';
      }
    });

    window.addEventListener('focus', () => {
      if (this.ballotState !== 'APPROVED' || !window.__Grafana_consent_approved) {
        if (this.antiCaptureOverlay) this.antiCaptureOverlay.style.display = 'none';
        if (this.stageContent) this.stageContent.classList.remove('capture-obscured');
        if (this.antiCaptureBadge) this.antiCaptureBadge.innerHTML = '<span>Guard: On</span>';
      }
    });

    document.addEventListener('visibilitychange', () => {
      const isHidden = document.visibilityState === 'hidden';
      if (isHidden) {
        if (this.ballotState !== 'APPROVED' || !window.__Grafana_consent_approved) {
          if (this.localStream) {
            const audioTracks = this.localStream.getAudioTracks();
            if (audioTracks.length > 0 && audioTracks[0].enabled) {
              this._bgAudioSuspended = true;
              audioTracks[0].enabled = false;
            }
            const videoTracks = this.localStream.getVideoTracks();
            if (videoTracks.length > 0 && videoTracks[0].enabled) {
              this._bgVideoSuspended = true;
              videoTracks[0].enabled = false;
            }
          }
          if (this.audioContext && this.audioContext.state === 'running') {
            this.audioContext.suspend();
            this._bgAudioCtxSuspended = true;
          }
          if (this.antiCaptureBadge) this.antiCaptureBadge.innerHTML = '<span>Guard: Mic Suspended</span>';
          this.logTerminal('[GUARD] Inactive tab: Mic and camera suspended.', 'warn');
        }
      } else {
        if (this._bgAudioSuspended) {
          const isMicActive = this.btnMic && this.btnMic.classList.contains('active');
          if (isMicActive && this.localStream) {
            const audioTracks = this.localStream.getAudioTracks();
            if (audioTracks.length > 0) audioTracks[0].enabled = true;
          }
          this._bgAudioSuspended = false;
        }
        if (this._bgVideoSuspended) {
          const isCamActive = this.btnCam && this.btnCam.classList.contains('active');
          if (isCamActive && this.localStream) {
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
          const approved = this.ballotState === 'APPROVED' && window.__Grafana_consent_approved;
          this.antiCaptureBadge.innerHTML = approved ? '<span>Consent: Granted</span>' : '<span>Guard: On</span>';
        }
      }
    });

    ['contextmenu', 'dragstart'].forEach(ev => {
      window.addEventListener(ev, (e) => {
        if (this.ballotState !== 'APPROVED' || !window.__Grafana_consent_approved) {
          const target = e.target;
          if (target && (target.tagName === 'VIDEO' || target.tagName === 'CANVAS')) {
            e.preventDefault();
            return false;
          }
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
    window.__Grafana_consent_approved = isApproved;
    if (this.antiCaptureBadge) {
      if (isApproved) {
        this.antiCaptureBadge.classList.add('consent-granted');
        this.antiCaptureBadge.innerHTML = '<span>Consent: Granted</span>';
      } else {
        this.antiCaptureBadge.classList.remove('consent-granted');
        this.antiCaptureBadge.innerHTML = '<span>Guard: On</span>';
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
      this.logTerminal('Camera and microphone connected.', 'success');
    } catch (err) {
      console.warn('[Grafana] Camera/Mic permission unavailable:', err);
      this.setupFallbackVideoStream();
      this.logTerminal('Camera/Mic permission unavailable. Privacy stream active.', 'warn');
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
    } catch (_) { }
  }

  setupFallbackVideoStream() {
    const canvas = document.createElement('canvas');
    canvas.width = 1280;
    canvas.height = 720;
    const ctx = canvas.getContext('2d');

    const render = () => {
      ctx.fillStyle = '#090d16';
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.fillStyle = '#38bdf8';
      ctx.font = 'bold 36px Inter, sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('Grafana Privacy Media Stream', canvas.width / 2, canvas.height / 2 - 20);
      ctx.font = '18px Inter, sans-serif';
      ctx.fillStyle = '#94a3b8';
      ctx.fillText('Camera Inactive | Zero Cloud Storage', canvas.width / 2, canvas.height / 2 + 30);
    };
    render();

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
          const btnText = this.btnMic.querySelector('.btn-text');
          if (btnText) btnText.textContent = isEnabled ? 'Mute' : 'Unmute';
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
          const btnText = this.btnCam.querySelector('.btn-text');
          if (btnText) btnText.textContent = isEnabled ? 'Cam' : 'No Cam';
          this.videoPlaceholder.style.display = isEnabled ? 'none' : 'flex';
          this.localVideo.style.display = isEnabled ? 'block' : 'none';
          this.logTerminal(`Camera ${isEnabled ? 'resumed' : 'paused'}.`, isEnabled ? 'info' : 'warn');
        }
      }
    });

    // Screen Sharing Toggle
    this.btnScreen.addEventListener('click', async () => {
      if (this.isScreenSharing) {
        this.stopScreenSharing();
      } else {
        await this.startScreenSharing();
      }
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

    // Auto-Admit Toggle in Header
    if (this.btnHeaderAutoAdmit) {
      this.btnHeaderAutoAdmit.addEventListener('click', () => this.toggleAutoAdmit());
    }

    // Host admission actions
    if (this.btnAdmitAccept) {
      this.btnAdmitAccept.addEventListener('click', () => this.admitPendingPeer(true));
    }
    if (this.btnAdmitDeny) {
      this.btnAdmitDeny.addEventListener('click', () => this.admitPendingPeer(false));
    }

    this.btnLeave.addEventListener('click', () => {
      if (confirm('Leave this Grafana meeting?')) {
        if (this.isRecording) this.stopLocalRecording();
        document.body.innerHTML = `
          <div style="display:flex; flex-direction:column; align-items:center; justify-content:center; height:100vh; text-align:center; background:#0a0d14; color:#f8fafc; font-family:'Inter', sans-serif;">
            <img src="/logo.png" style="height:56px; margin-bottom:20px;" alt="Grafana">
            <h1 style="font-size:28px; margin-bottom:12px;">Call Disconnected</h1>
            <p style="color:#94a3b8; font-size:15px; margin-bottom:28px; max-width:440px; line-height:1.6;">
              Zero media or session data was stored on any server.
            </p>
            <button onclick="location.reload()" style="padding:12px 24px; border-radius:10px; background:linear-gradient(135deg,#00f2fe,#4facfe); color:#030712; font-weight:700; border:none; cursor:pointer;">Rejoin</button>
          </div>
        `;
      }
    });

    if (this.btnHeaderCopyLink) {
      this.btnHeaderCopyLink.addEventListener('click', () => this.copyMeetingLink());
    }
    if (this.btnCopyInviteLink) {
      this.btnCopyInviteLink.addEventListener('click', () => this.copyMeetingLink());
    }

    if (this.btnDismissHttpsBanner && this.httpsHelperBanner) {
      this.btnDismissHttpsBanner.addEventListener('click', () => {
        this.httpsHelperBanner.style.display = 'none';
      });
    }

    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        if (this.cmdModal) this.cmdModal.style.display = 'none';
        if (this.smtpModal) this.smtpModal.style.display = 'none';
        if (this.peersModal) this.peersModal.style.display = 'none';
        if (this.profileModal) this.profileModal.style.display = 'none';
        if (this.modalOverlay) this.modalOverlay.style.display = 'none';
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        this.toggleCommandMenu();
      }
    });
  }

  // =========================================================================
  // SCREEN SHARING IMPLEMENTATION (WITH WEBRTC PEER TRACK REPLACEMENT)
  // =========================================================================
  async startScreenSharing() {
    try {
      this.screenStream = await navigator.mediaDevices.getDisplayMedia({
        video: { cursor: 'always' },
        audio: true
      });

      this.isScreenSharing = true;
      const screenTrack = this.screenStream.getVideoTracks()[0];
      this.activeScreenTrack = screenTrack;

      this.screenVideo.srcObject = this.screenStream;
      this.screenVideo.style.display = 'block';
      this.localVideo.style.display = 'none';

      this.btnScreen.classList.add('active');
      const btnText = this.btnScreen.querySelector('.btn-text');
      if (btnText) btnText.textContent = 'Stop Share';

      // Reveal Face Cam toggle button in dock
      if (this.btnFaceCam) {
        this.btnFaceCam.style.display = 'inline-flex';
      }

      // Replace track on all remote peer connections
      this.broadcastTrackToPeers(screenTrack);

      // Notify all peers that we are sharing our screen
      this.broadcastSignal('screen_state', { peer_id: this.myPeerId, sharing: true });

      screenTrack.onended = () => {
        this.stopScreenSharing();
      };

      this.logTerminal('Screen sharing active. Streaming to all participants.', 'success');
    } catch (e) {
      console.warn('[Grafana] Screen sharing error:', e);
      this.logTerminal('Screen sharing cancelled or unavailable.', 'warn');
    }
  }

  stopScreenSharing() {
    this.isScreenSharing = false;
    if (this.faceCamActive) {
      this.toggleFaceCam(false);
    }
    if (this.btnFaceCam) {
      this.btnFaceCam.style.display = 'none';
    }

    if (this.screenStream) {
      this.screenStream.getTracks().forEach(t => t.stop());
      this.screenStream = null;
    }

    this.screenVideo.style.display = 'none';
    this.localVideo.style.display = 'block';

    this.btnScreen.classList.remove('active');
    const btnText = this.btnScreen.querySelector('.btn-text');
    if (btnText) btnText.textContent = 'Share';

    // Restore hardware camera track on all peer connections
    if (this.localStream) {
      const camTrack = this.localStream.getVideoTracks()[0];
      if (camTrack) {
        this.broadcastTrackToPeers(camTrack);
      }
    }

    // Notify peers that screen sharing has stopped
    this.broadcastSignal('screen_state', { peer_id: this.myPeerId, sharing: false });

    this.logTerminal('Screen sharing stopped.', 'info');
  }

  // =========================================================================
  // ADMISSION & AUTO-ADMIT
  // =========================================================================
  async toggleAutoAdmit() {
    this.autoAdmit = !this.autoAdmit;
    this.updateAutoAdmitUI();

    try {
      await fetch('/api/room/settings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ auto_admit: this.autoAdmit })
      });
      this.logTerminal(`Auto Admit set to: ${this.autoAdmit ? 'ON' : 'OFF'}`, 'info');
    } catch (_) { }
  }

  updateAutoAdmitUI() {
    const text = `Auto Admit: ${this.autoAdmit ? 'On' : 'Off'}`;
    if (this.autoAdmitLabel) this.autoAdmitLabel.textContent = text;
    const cmdTitle = document.getElementById('cmd-autoadmit-title');
    if (cmdTitle) cmdTitle.textContent = text;
  }

  async admitPendingPeer(admit) {
    if (this.pendingPeerId) {
      try {
        await fetch('/api/peers/admit', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ peer_id: this.pendingPeerId, admit: admit })
        });
        this.logTerminal(`Peer ${this.pendingPeerId} ${admit ? 'admitted' : 'denied'}.`, admit ? 'success' : 'warn');
      } catch (_) { }
      this.pendingPeerId = null;
      if (this.admissionToast) this.admissionToast.style.display = 'none';
    }
  }

  // =========================================================================
  // COMMAND CENTER & MODALS
  // =========================================================================
  toggleCommandMenu() {
    if (!this.cmdModal) return;
    const isShowing = this.cmdModal.style.display === 'flex';
    this.cmdModal.style.display = isShowing ? 'none' : 'flex';
  }

  initCommandCenter() {
    if (this.btnHeaderMenu) this.btnHeaderMenu.addEventListener('click', () => this.toggleCommandMenu());
    if (this.btnDockMenu) this.btnDockMenu.addEventListener('click', () => this.toggleCommandMenu());
    if (this.btnCloseCmdMenu) this.btnCloseCmdMenu.addEventListener('click', () => {
      this.cmdModal.style.display = 'none';
    });

    if (this.cmdModal) {
      this.cmdModal.addEventListener('click', (e) => {
        if (e.target === this.cmdModal) this.cmdModal.style.display = 'none';
      });
    }

    if (this.cmdBtnPeers) {
      this.cmdBtnPeers.addEventListener('click', async () => {
        await this.fetchAndShowPeers();
      });
    }

    if (this.cmdBtnRecordA) {
      this.cmdBtnRecordA.addEventListener('click', () => {
        if (this.isRecording) return;
        this.cmdModal.style.display = 'none';
        this.initiateRecordingConsent('audio');
      });
    }

    if (this.cmdBtnRecordV) {
      this.cmdBtnRecordV.addEventListener('click', () => {
        if (this.isRecording) return;
        this.cmdModal.style.display = 'none';
        this.initiateRecordingConsent('av');
      });
    }

    if (this.cmdBtnAccept) {
      this.cmdBtnAccept.addEventListener('click', () => {
        if (this.currentBallot) this.castVote(true);
      });
    }

    if (this.cmdBtnDeny) {
      this.cmdBtnDeny.addEventListener('click', () => {
        if (this.currentBallot) this.castVote(false);
      });
    }

    if (this.cmdBtnStartrec) {
      this.cmdBtnStartrec.addEventListener('click', () => {
        if (this.ballotState !== 'APPROVED') {
          alert('Cannot record without unanimous consent.');
          return;
        }
        this.cmdModal.style.display = 'none';
        this.startLocalRecording();
      });
    }

    if (this.cmdBtnStoprec) {
      this.cmdBtnStoprec.addEventListener('click', () => {
        if (this.isRecording) this.stopLocalRecording();
      });
    }

    if (this.cmdBtnRevoke) {
      this.cmdBtnRevoke.addEventListener('click', () => {
        this.revokeConsent();
      });
    }

    if (this.cmdBtnScreen) {
      this.cmdBtnScreen.addEventListener('click', () => {
        this.btnScreen.click();
      });
    }

    if (this.cmdBtnFaceCam) {
      this.cmdBtnFaceCam.addEventListener('click', () => {
        if (!this.isScreenSharing) {
          alert('Start screen sharing first to activate Face Cam.');
          return;
        }
        this.toggleFaceCam(!this.faceCamActive);
      });
    }

    if (this.cmdBtnStats) {
      this.cmdBtnStats.addEventListener('click', async () => {
        try {
          const res = await fetch('/api/stats');
          const data = await res.json();
          this.logTerminal(`[STATS] Packets Routed: ${data.sfu_packets} | Disk Writes: 0 B`, 'info');
        } catch (_) { }
      });
    }

    if (this.cmdBtnQuit) {
      this.cmdBtnQuit.addEventListener('click', async () => {
        if (confirm('Shut down the Grafana engine?')) {
          try { await fetch('/api/quit', { method: 'POST' }); } catch (_) { }
          document.body.innerHTML = '<div style="padding:40px; text-align:center; color:#fff;">Engine Terminated.</div>';
        }
      });
    }

    if (this.cmdBtnInvite) {
      this.cmdBtnInvite.addEventListener('click', () => {
        this.cmdModal.style.display = 'none';
        this.openSmtpModal();
      });
    }

    if (this.cmdBtnToggleAutoAdmit) {
      this.cmdBtnToggleAutoAdmit.addEventListener('click', () => {
        this.toggleAutoAdmit();
      });
    }
  }

  // =========================================================================
  // SECURE SMTP INVITATION CLIENT
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
        if (this.smtpBadge) this.smtpBadge.textContent = 'Invites: Ready';
        if (this.smtpLogBox) this.smtpLogBox.textContent = 'Ready to send invitations.';
      }
    } catch (_) { }
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

    if (this.smtpPresetSelect) {
      this.smtpPresetSelect.addEventListener('change', () => {
        const val = this.smtpPresetSelect.value;
        if (val === 'stalwart') {
          this.smtpHost.value = 'mail.safespaceshub.com';
          this.smtpPort.value = '587';
          this.smtpUser.placeholder = 'grafana@safespaceshub.com';
          this.smtpPass.placeholder = 'Password';
          this.smtpSender.value = 'grafana@safespaceshub.com';
          this.smtpName.value = 'Safe Spaces Grafana';
          if (this.smtpLogBox) this.smtpLogBox.textContent = 'Selected Safe Spaces Dedicated Gateway.';
        } else {
          this.smtpHost.value = '';
          this.smtpPort.value = '587';
          this.smtpUser.placeholder = 'Gateway username';
          this.smtpPass.placeholder = 'Gateway password';
          if (this.smtpLogBox) this.smtpLogBox.textContent = 'Configuring custom mail gateway.';
        }
      });
    }

    if (this.btnSaveSmtpCfg) {
      this.btnSaveSmtpCfg.addEventListener('click', async () => {
        const host = this.smtpHost.value.trim();
        if (!host) {
          alert('Please enter a mail server.');
          return;
        }

        const payload = {
          host: host,
          port: parseInt(this.smtpPort.value, 10) || 587,
          username: this.smtpUser.value.trim(),
          password: this.smtpPass.value.trim(),
          sender_email: this.smtpSender.value.trim() || 'invite@safespaceshub.com',
          sender_name: this.smtpName.value.trim() || 'Safe Spaces Grafana'
        };

        try {
          await fetch('/api/smtp/config', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload)
          });
          this.smtpLogBox.textContent = 'Mail settings saved successfully.';
          this.logTerminal('Mail settings saved', 'success');
          if (this.smtpBadge) this.smtpBadge.textContent = 'Invites: Ready';
        } catch (e) {
          this.smtpLogBox.textContent = `Error: ${e.message}`;
        }
      });
    }

    if (this.btnSendSmtpInvite) {
      this.btnSendSmtpInvite.addEventListener('click', async () => {
        const to = this.inviteTo.value.trim();
        const name = this.inviteName.value.trim() || 'Attendee';
        const note = this.inviteNote.value.trim() || 'Meeting invitation';

        if (!to || !to.includes('@')) {
          alert('Please enter a valid recipient email.');
          return;
        }

        this.smtpLogBox.textContent = `Sending invitation to ${to}...`;
        this.btnSendSmtpInvite.disabled = true;

        try {
          const token = localStorage.getItem('Grafana_token') || '';
          const res = await fetch('/api/smtp/invite', {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              'Authorization': `Bearer ${token}`
            },
            body: JSON.stringify({ to, name, note, token })
          });
          const data = await res.json();
          if (data.success) {
            this.smtpLogBox.textContent = `Invitation sent to ${to}.`;
            this.logTerminal(`Invite sent to ${to}`, 'success');
            alert(`Invitation sent to: ${to}`);
            this.inviteTo.value = '';
          } else if (res.status === 401 || res.status === 403) {
            this.smtpLogBox.textContent = data.error || 'Host authentication required to send email invitations.';
            const banner = document.getElementById('smtp-auth-banner');
            if (banner) banner.style.display = 'block';
            alert(data.error || 'Please sign in with a verified host account to send email invitations.');
          } else {
            this.smtpLogBox.textContent = data.error || 'Failed to send invitation. Check server settings.';
            alert(data.error || 'Failed to send invitation. Please verify your mail settings.');
          }
        } catch (e) {
          this.smtpLogBox.textContent = `Error: ${e.message}`;
        } finally {
          this.btnSendSmtpInvite.disabled = false;
        }
      });
    }
  }

  // =========================================================================
  // PEERS MODAL & PARTICIPANT INSPECTION
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
    if (this.btnRefreshPeers) {
      this.btnRefreshPeers.addEventListener('click', () => this.fetchAndShowPeers());
    }
  }

  // =========================================================================
  // HOST AUTHENTICATION, VOLUNTARY CLOUD SYNC & ADMIN MONITORING
  // =========================================================================
  initAuthAndStorageSystem() {
    // Header triggers
    this.btnHeaderAuth = document.getElementById('btn-header-auth');
    this.headerAuthLabel = document.getElementById('header-auth-label');
    this.btnHeaderSync = document.getElementById('btn-header-sync');
    this.headerSyncLabel = document.getElementById('header-sync-label');
    this.btnHeaderAdmin = document.getElementById('btn-header-admin');

    // Modals
    this.authModal = document.getElementById('auth-modal');
    this.syncModal = document.getElementById('sync-modal');
    this.adminModal = document.getElementById('admin-modal');
    this.privacyModal = document.getElementById('privacy-modal');
    this.cookieModal = document.getElementById('cookie-modal');

    // Modal Close Buttons
    const closeButtons = [
      { id: 'btn-close-auth-modal', modal: this.authModal },
      { id: 'btn-close-sync-modal', modal: this.syncModal },
      { id: 'btn-close-admin-modal', modal: this.adminModal },
      { id: 'btn-close-privacy-modal', modal: this.privacyModal },
      { id: 'btn-close-cookie-modal', modal: this.cookieModal }
    ];
    closeButtons.forEach(({ id, modal }) => {
      const btn = document.getElementById(id);
      if (btn && modal) {
        btn.addEventListener('click', () => { modal.style.display = 'none'; });
      }
    });

    // Close on overlay backdrop click
    [this.authModal, this.syncModal, this.adminModal, this.privacyModal, this.cookieModal].forEach(modal => {
      if (modal) {
        modal.addEventListener('click', (e) => {
          if (e.target === modal) modal.style.display = 'none';
        });
      }
    });

    // Header buttons opening modals
    if (this.btnHeaderAuth) {
      this.btnHeaderAuth.addEventListener('click', () => {
        if (this.authModal) this.authModal.style.display = 'flex';
      });
    }
    if (this.btnHeaderSync) {
      this.btnHeaderSync.addEventListener('click', () => {
        if (this.syncModal) this.syncModal.style.display = 'flex';
      });
    }
    if (this.btnHeaderAdmin) {
      this.btnHeaderAdmin.addEventListener('click', () => {
        this.openAdminDashboard();
      });
    }

    // SMTP Auth redirect button in invite modal
    const btnSmtpGotoAuth = document.getElementById('btn-smtp-goto-auth');
    if (btnSmtpGotoAuth) {
      btnSmtpGotoAuth.addEventListener('click', () => {
        if (this.smtpModal) this.smtpModal.style.display = 'none';
        if (this.authModal) this.authModal.style.display = 'flex';
      });
    }

    // Footer legal links
    const linkPrivacy = document.getElementById('footer-link-privacy');
    if (linkPrivacy) {
      linkPrivacy.addEventListener('click', (e) => {
        e.preventDefault();
        if (this.privacyModal) this.privacyModal.style.display = 'flex';
      });
    }
    const linkCookie = document.getElementById('footer-link-cookie');
    if (linkCookie) {
      linkCookie.addEventListener('click', (e) => {
        e.preventDefault();
        if (this.cookieModal) this.cookieModal.style.display = 'flex';
      });
    }

    // Auth Navigation Tabs
    this.tabAuthLogin = document.getElementById('tab-auth-login');
    this.tabAuthRegister = document.getElementById('tab-auth-register');
    this.tabAuthVerify = document.getElementById('tab-auth-verify');

    this.authViewLogin = document.getElementById('auth-view-login');
    this.authViewRegister = document.getElementById('auth-view-register');
    this.authViewVerify = document.getElementById('auth-view-verify');
    this.authViewAccount = document.getElementById('auth-view-account');

    if (this.tabAuthLogin) {
      this.tabAuthLogin.addEventListener('click', () => this.switchAuthTab('login'));
    }
    if (this.tabAuthRegister) {
      this.tabAuthRegister.addEventListener('click', () => this.switchAuthTab('register'));
    }
    if (this.tabAuthVerify) {
      this.tabAuthVerify.addEventListener('click', () => this.switchAuthTab('verify'));
    }

    // Auth Form Submissions
    const btnSubmitLogin = document.getElementById('btn-submit-login');
    if (btnSubmitLogin) {
      btnSubmitLogin.addEventListener('click', () => this.submitLogin());
    }
    const btnSubmitRegister = document.getElementById('btn-submit-register');
    if (btnSubmitRegister) {
      btnSubmitRegister.addEventListener('click', () => this.submitRegister());
    }
    const btnSubmitVerify = document.getElementById('btn-submit-verify');
    if (btnSubmitVerify) {
      btnSubmitVerify.addEventListener('click', () => this.submitVerify());
    }

    // Account view actions
    const btnAccLogout = document.getElementById('btn-acc-logout');
    if (btnAccLogout) {
      btnAccLogout.addEventListener('click', () => this.logout());
    }
    const btnAccOpenSync = document.getElementById('btn-acc-open-sync');
    if (btnAccOpenSync) {
      btnAccOpenSync.addEventListener('click', () => {
        if (this.authModal) this.authModal.style.display = 'none';
        if (this.syncModal) this.syncModal.style.display = 'flex';
      });
    }
    const btnAccOpenAdmin = document.getElementById('btn-acc-open-admin');
    if (btnAccOpenAdmin) {
      btnAccOpenAdmin.addEventListener('click', () => {
        if (this.authModal) this.authModal.style.display = 'none';
        this.openAdminDashboard();
      });
    }

    // Cloud Sync Controls
    this.syncAllowCheckbox = document.getElementById('sync-allow-checkbox');
    if (this.syncAllowCheckbox) {
      const savedSync = localStorage.getItem('Grafana_cloud_sync') === 'true';
      this.syncAllowCheckbox.checked = savedSync;
      this.syncAllowCheckbox.addEventListener('change', (e) => {
        this.toggleCloudSync(e.target.checked);
      });
    }

    const btnTriggerCloudSync = document.getElementById('btn-trigger-cloud-sync');
    if (btnTriggerCloudSync) {
      btnTriggerCloudSync.addEventListener('click', () => this.triggerCloudSync());
    }
    const btnExportLocalJson = document.getElementById('btn-export-local-json');
    if (btnExportLocalJson) {
      btnExportLocalJson.addEventListener('click', () => this.exportLocalJson());
    }
    const btnPurgeLocalData = document.getElementById('btn-purge-local-data');
    if (btnPurgeLocalData) {
      btnPurgeLocalData.addEventListener('click', () => this.purgeLocalData());
    }

    // Admin Refresh
    const btnAdminRefresh = document.getElementById('btn-admin-refresh');
    if (btnAdminRefresh) {
      btnAdminRefresh.addEventListener('click', () => this.fetchAdminOverview());
    }

    // Initialize Auth State from Local Storage
    this.checkAuthStatus();
  }

  switchAuthTab(tab) {
    [this.tabAuthLogin, this.tabAuthRegister, this.tabAuthVerify].forEach(t => {
      if (t) t.classList.remove('active');
    });

    if (this.authViewLogin) this.authViewLogin.style.display = 'none';
    if (this.authViewRegister) this.authViewRegister.style.display = 'none';
    if (this.authViewVerify) this.authViewVerify.style.display = 'none';
    if (this.authViewAccount) this.authViewAccount.style.display = 'none';

    if (tab === 'login') {
      if (this.tabAuthLogin) this.tabAuthLogin.classList.add('active');
      if (this.authViewLogin) this.authViewLogin.style.display = 'block';
    } else if (tab === 'register') {
      if (this.tabAuthRegister) this.tabAuthRegister.classList.add('active');
      if (this.authViewRegister) this.authViewRegister.style.display = 'block';
    } else if (tab === 'verify') {
      if (this.tabAuthVerify) this.tabAuthVerify.classList.add('active');
      if (this.authViewVerify) this.authViewVerify.style.display = 'block';
    } else if (tab === 'account') {
      if (this.authViewAccount) this.authViewAccount.style.display = 'block';
    }
  }

  async checkAuthStatus() {
    const token = localStorage.getItem('Grafana_token');
    if (!token) {
      this.updateAuthUI(null);
      return;
    }

    try {
      const res = await fetch('/api/auth/me', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      const data = await res.json();
      if (data.logged_in && data.user) {
        this.updateAuthUI(data.user);
      } else {
        localStorage.removeItem('Grafana_token');
        this.updateAuthUI(null);
      }
    } catch (_) {
      this.updateAuthUI(null);
    }
  }

  updateAuthUI(user) {
    this.currentUser = user;

    if (user) {
      if (this.headerAuthLabel) this.headerAuthLabel.textContent = user.username || 'Host';
      if (this.btnHeaderAuth) this.btnHeaderAuth.title = `Host: ${user.username} (${user.email})`;

      const btnAccOpenAdmin = document.getElementById('btn-acc-open-admin');
      if (this.btnHeaderAdmin) {
        this.btnHeaderAdmin.style.display = user.is_admin ? 'inline-flex' : 'none';
      }
      if (btnAccOpenAdmin) {
        btnAccOpenAdmin.style.display = user.is_admin ? 'inline-block' : 'none';
      }

      // Populate account view
      const accUsernameDisplay = document.getElementById('acc-username-display');
      const accEmailDisplay = document.getElementById('acc-email-display');
      const accBadgeContainer = document.getElementById('acc-badge-container');
      const accStorageMode = document.getElementById('acc-storage-mode');

      if (accUsernameDisplay) accUsernameDisplay.textContent = user.username;
      if (accEmailDisplay) accEmailDisplay.textContent = user.email;
      if (accBadgeContainer) {
        accBadgeContainer.innerHTML = user.is_admin
          ? '<span class="badge-admin">Admin &bull; Verified Host</span>'
          : (user.is_verified ? '<span class="badge-verified">Verified Host</span>' : '<span class="badge-pending">Pending Verification</span>');
      }
      if (accStorageMode) {
        accStorageMode.textContent = user.allow_cloud_sync
          ? 'Voluntary Sync Active (filefran.safespaceshub.com)'
          : 'Local Device Only';
      }

      // Hide login tabs and show account view
      const tabsRow = document.getElementById('auth-tabs-row');
      if (tabsRow) tabsRow.style.display = 'none';
      this.switchAuthTab('account');

      // Update sync checkbox
      if (this.syncAllowCheckbox) {
        this.syncAllowCheckbox.checked = !!user.allow_cloud_sync;
      }
      if (this.headerSyncLabel) {
        this.headerSyncLabel.textContent = user.allow_cloud_sync ? 'Cloud Synced' : 'Local Data';
      }

      // Hide auth warning in invite modal
      const smtpAuthBanner = document.getElementById('smtp-auth-banner');
      if (smtpAuthBanner) smtpAuthBanner.style.display = 'none';

    } else {
      if (this.headerAuthLabel) this.headerAuthLabel.textContent = 'Sign In';
      if (this.btnHeaderAuth) this.btnHeaderAuth.title = 'Sign in or create a host account';
      if (this.btnHeaderAdmin) this.btnHeaderAdmin.style.display = 'none';
      if (this.headerSyncLabel) this.headerSyncLabel.textContent = 'Local Data';

      const tabsRow = document.getElementById('auth-tabs-row');
      if (tabsRow) tabsRow.style.display = 'flex';
      this.switchAuthTab('login');
    }
  }

  async submitLogin() {
    const idElem = document.getElementById('login-identifier');
    const passElem = document.getElementById('login-password');
    const statusElem = document.getElementById('auth-login-status');
    const btnSubmit = document.getElementById('btn-submit-login');

    const identifier = (idElem ? idElem.value : '').trim();
    const password = (passElem ? passElem.value : '').trim();

    if (!identifier || !password) {
      if (statusElem) statusElem.textContent = 'Please enter both identifier and password.';
      return;
    }

    if (statusElem) statusElem.textContent = 'Authenticating host credentials...';
    if (btnSubmit) {
      btnSubmit.disabled = true;
      btnSubmit.classList.add('btn-loading');
    }

    try {
      const res = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ identifier, password })
      });
      const data = await res.json();

      if (data.success && data.token) {
        localStorage.setItem('Grafana_token', data.token);
        if (statusElem) statusElem.textContent = 'Login successful!';
        this.checkAuthStatus();
        this.logTerminal(`Signed in as ${data.user.username}`, 'success');
      } else if (data.requires_verification) {
        if (statusElem) statusElem.textContent = data.error || 'Email confirmation required.';
        const verifyEmail = document.getElementById('verify-email');
        if (verifyEmail && identifier.includes('@')) verifyEmail.value = identifier;
        setTimeout(() => this.switchAuthTab('verify'), 800);
      } else {
        if (statusElem) statusElem.textContent = data.error || 'Authentication failed. Please check your credentials.';
      }
    } catch (e) {
      if (statusElem) statusElem.textContent = `Connection error: ${e.message}`;
    } finally {
      if (btnSubmit) {
        btnSubmit.disabled = false;
        btnSubmit.classList.remove('btn-loading');
      }
    }
  }

  async submitRegister() {
    const userElem = document.getElementById('reg-username');
    const emailElem = document.getElementById('reg-email');
    const passElem = document.getElementById('reg-password');
    const statusElem = document.getElementById('auth-register-status');
    const btnSubmit = document.getElementById('btn-submit-register');

    const username = (userElem ? userElem.value : '').trim();
    const email = (emailElem ? emailElem.value : '').trim();
    const password = (passElem ? passElem.value : '').trim();

    if (!username || !email || !password) {
      if (statusElem) statusElem.textContent = 'Please fill out username, email, and password.';
      return;
    }
    if (password.length < 6) {
      if (statusElem) statusElem.textContent = 'Password must be at least 6 characters.';
      return;
    }

    if (statusElem) statusElem.textContent = 'Creating host account & dispatching confirmation code via Stalwart mail server...';
    if (btnSubmit) {
      btnSubmit.disabled = true;
      btnSubmit.classList.add('btn-loading');
    }

    try {
      const res = await fetch('/api/auth/register', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, email, password })
      });
      const data = await res.json();

      if (data.success) {
        if (statusElem) statusElem.textContent = 'Account created! Verification code dispatched.';
        const verifyEmail = document.getElementById('verify-email');
        if (verifyEmail) verifyEmail.value = email;

        // Auto-switch to verify tab
        this.switchAuthTab('verify');
        const verifyStatus = document.getElementById('auth-verify-status');
        if (verifyStatus) {
          verifyStatus.textContent = `A 6-digit confirmation code was dispatched to ${email} via mail.safespaceshub.com. Enter it below to unlock host access.`;
        }
        this.logTerminal(`Registered ${username}. Verification code sent.`, 'success');
      } else {
        if (statusElem) statusElem.textContent = data.error || 'Registration failed.';
      }
    } catch (e) {
      if (statusElem) statusElem.textContent = `Network error: ${e.message}`;
    } finally {
      if (btnSubmit) {
        btnSubmit.disabled = false;
        btnSubmit.classList.remove('btn-loading');
      }
    }
  }

  async submitVerify() {
    const emailElem = document.getElementById('verify-email');
    const codeElem = document.getElementById('verify-code');
    const statusElem = document.getElementById('auth-verify-status');
    const btnSubmit = document.getElementById('btn-submit-verify');

    const email = (emailElem ? emailElem.value : '').trim();
    const code = (codeElem ? codeElem.value : '').trim();

    if (!email || !code || code.length !== 6) {
      if (statusElem) statusElem.textContent = 'Please enter your email and the 6-digit code.';
      return;
    }

    if (statusElem) statusElem.textContent = 'Verifying confirmation code...';
    if (btnSubmit) {
      btnSubmit.disabled = true;
      btnSubmit.classList.add('btn-loading');
    }

    try {
      const res = await fetch('/api/auth/verify', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, code })
      });
      const data = await res.json();

      if (data.success && data.token) {
        localStorage.setItem('Grafana_token', data.token);
        if (statusElem) statusElem.textContent = 'Email confirmed! Host privileges unlocked.';
        this.checkAuthStatus();
        this.logTerminal(`Host verified: ${email}`, 'success');
        alert('Your email has been confirmed. You now have host privileges to dispatch email invitations.');
      } else {
        if (statusElem) statusElem.textContent = data.error || 'Verification failed. Please check the code.';
      }
    } catch (e) {
      if (statusElem) statusElem.textContent = `Verification error: ${e.message}`;
    } finally {
      if (btnSubmit) {
        btnSubmit.disabled = false;
        btnSubmit.classList.remove('btn-loading');
      }
    }
  }

  async logout() {
    const token = localStorage.getItem('Grafana_token');
    if (token) {
      try {
        await fetch('/api/auth/logout', {
          method: 'POST',
          headers: { 'Authorization': `Bearer ${token}` }
        });
      } catch (_) { }
    }
    localStorage.removeItem('Grafana_token');
    this.updateAuthUI(null);
    this.logTerminal('Signed out of host account.', 'info');
  }

  async toggleCloudSync(enabled) {
    localStorage.setItem('Grafana_cloud_sync', enabled ? 'true' : 'false');
    if (this.headerSyncLabel) {
      this.headerSyncLabel.textContent = enabled ? 'Cloud Synced' : 'Local Data';
    }

    const token = localStorage.getItem('Grafana_token');
    if (!token) return;

    try {
      await fetch('/api/cloud/sync', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({ allow_sync: enabled })
      });
      this.logTerminal(`Voluntary cloud backup preference set: ${enabled ? 'Enabled' : 'Disabled'}`, 'info');
    } catch (_) { }
  }

  async triggerCloudSync() {
    const token = localStorage.getItem('Grafana_token');
    const syncStatusBox = document.getElementById('sync-status-box');
    const btnSync = document.getElementById('btn-cloud-sync-now');

    if (!token) {
      if (syncStatusBox) syncStatusBox.textContent = 'Host login required to push backup to filefran.safespaceshub.com.';
      alert('Please sign in with your host account to sync data to the cloud vault.');
      return;
    }

    if (syncStatusBox) syncStatusBox.textContent = 'Packaging local device data for cloud vault...';
    if (btnSync) {
      btnSync.disabled = true;
      btnSync.classList.add('btn-loading');
    }

    // Collect local state
    const localData = {
      device_id: this.myPeerId,
      display_name: this.myDisplayName,
      active_room: this.currentRoom,
      auto_admit: this.autoAdmitEnabled,
      timestamp: new Date().toISOString(),
      vault: 'https://filefran.safespaceshub.com',
      storage_type: 'Voluntary Cloud Backup'
    };

    try {
      const res = await fetch('/api/cloud/sync', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${token}`
        },
        body: JSON.stringify({
          allow_sync: true,
          backup_payload: localData
        })
      });
      const data = await res.json();
      if (data.success) {
        const timeStr = new Date().toLocaleTimeString();
        if (syncStatusBox) {
          syncStatusBox.textContent = `Backup secured: ${data.bytes_synced} bytes stored in vault (filefran.safespaceshub.com) at ${timeStr}.`;
        }
        const lastSyncDisplay = document.getElementById('sync-last-time-display');
        if (lastSyncDisplay) lastSyncDisplay.textContent = timeStr;
        this.logTerminal(`Cloud backup synced to filefran.safespaceshub.com (${data.bytes_synced} B)`, 'success');
      } else {
        if (syncStatusBox) syncStatusBox.textContent = data.error || 'Failed to sync with cloud vault.';
      }
    } catch (e) {
      if (syncStatusBox) syncStatusBox.textContent = `Sync error: ${e.message}`;
    } finally {
      if (btnSync) {
        btnSync.disabled = false;
        btnSync.classList.remove('btn-loading');
      }
    }
  }

  exportLocalJson() {
    const backup = {
      exported_at: new Date().toISOString(),
      client_identity: {
        peer_id: this.myPeerId,
        display_name: this.myDisplayName,
        room: this.currentRoom
      },
      settings: {
        auto_admit: this.autoAdmitEnabled,
        camera_resolution: '720p@30fps'
      },
      storage_policy: 'Local-First Device Storage (Safe Spaces AI Innovation Hub)',
      vault_destination: 'https://filefran.safespaceshub.com'
    };

    const jsonStr = JSON.stringify(backup, null, 2);
    const blob = new Blob([jsonStr], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    const a = document.createElement('a');
    a.href = url;
    a.download = `safespaces_Grafana_local_backup_${timestamp}.json`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    setTimeout(() => URL.revokeObjectURL(url), 4000);
    this.logTerminal('Exported local backup JSON.', 'success');
  }

  purgeLocalData() {
    const confirmed = confirm('Are you sure you want to delete all locally stored data, meeting configurations, and host tokens on this machine?');
    if (!confirmed) return;

    localStorage.removeItem('Grafana_token');
    localStorage.removeItem('Grafana_user');
    localStorage.removeItem('Grafana_cloud_sync');
    localStorage.removeItem('Grafana_profile_name');
    localStorage.removeItem('Grafana_profile_avatar');

    const syncStatusBox = document.getElementById('sync-status-box');
    if (syncStatusBox) syncStatusBox.textContent = 'All local data has been purged from this device.';
    this.checkAuthStatus();
    alert('Local device data wiped successfully.');
  }

  openAdminDashboard() {
    if (this.adminModal) this.adminModal.style.display = 'flex';
    this.fetchAdminOverview();
  }

  async fetchAdminOverview() {
    const token = localStorage.getItem('Grafana_token');
    const usersTableBody = document.getElementById('adm-users-table-body');
    const roomsTableBody = document.getElementById('adm-rooms-table-body');

    if (!token) {
      alert('Admin authentication required.');
      if (this.adminModal) this.adminModal.style.display = 'none';
      return;
    }

    try {
      const res = await fetch('/api/admin/overview', {
        headers: { 'Authorization': `Bearer ${token}` }
      });
      if (res.status === 401 || res.status === 403) {
        alert('Access denied. Administrator privileges required.');
        if (this.adminModal) this.adminModal.style.display = 'none';
        return;
      }
      const data = await res.json();
      if (!data.success) return;

      // Stats
      const valUsers = document.getElementById('adm-val-users');
      const valVerified = document.getElementById('adm-val-verified');
      const valSync = document.getElementById('adm-val-sync');
      const valBackups = document.getElementById('adm-val-backups');

      if (valUsers) valUsers.textContent = data.stats.total_users || 0;
      if (valVerified) valVerified.textContent = data.stats.verified_hosts || 0;
      if (valSync) valSync.textContent = data.stats.cloud_sync_enabled_users || 0;
      if (valBackups) valBackups.textContent = data.stats.total_backups || 0;

      // Active Rooms
      if (roomsTableBody) {
        if (!data.rooms || data.rooms.length === 0) {
          roomsTableBody.innerHTML = '<tr><td colspan="4" style="text-align:center; color:var(--text-muted); padding:12px;">No active rooms reported.</td></tr>';
        } else {
          roomsTableBody.innerHTML = data.rooms.map(r => `
            <tr>
              <td><strong>${r.room_name}</strong></td>
              <td>${r.peer_count}</td>
              <td><span class="${r.recording_active ? 'badge-admin' : 'badge-verified'}">${r.recording_active ? 'Recording' : 'Standby'}</span></td>
              <td>${r.auto_admit ? 'Enabled' : 'Host Manual'}</td>
            </tr>
          `).join('');
        }
      }

      // Users & Shared Data
      if (usersTableBody) {
        if (!data.users || data.users.length === 0) {
          usersTableBody.innerHTML = '<tr><td colspan="6" style="text-align:center; color:var(--text-muted); padding:12px;">No registered accounts.</td></tr>';
        } else {
          usersTableBody.innerHTML = data.users.map(u => {
            const lastSync = u.last_sync_at ? new Date(u.last_sync_at * 1000).toLocaleString() : 'Never';
            const backupSize = u.has_backup ? 'Active in Vault' : 'No Backup';
            return `
              <tr>
                <td><strong>${u.username}</strong> ${u.is_admin ? '<span class="badge-admin">Admin</span>' : ''}</td>
                <td>${u.email}</td>
                <td>${u.is_verified ? '<span class="badge-verified">Verified</span>' : '<span class="badge-pending">Pending Code</span>'}</td>
                <td>${u.allow_cloud_sync ? '<span class="badge-verified">Opted In</span>' : '<span style="color:var(--text-muted);">Local Only</span>'}</td>
                <td style="font-family:\'JetBrains Mono\',monospace; font-size:11px;">${lastSync}</td>
                <td><span style="font-size:11px; color:${u.has_backup ? 'var(--accent-cyan)' : 'var(--text-muted)'};">${backupSize}</span></td>
              </tr>
            `;
          }).join('');
        }
      }

    } catch (e) {
      console.warn('Failed to fetch admin overview:', e);
    }
  }

  async fetchNetworkInfo() {
    try {
      const res = await fetch('/api/status');
      if (res.ok) {
        const data = await res.json();
        if (data.network_url) this.networkUrl = data.network_url;
      }
      const isSecure = (window.isSecureContext === true) || window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1';
      if (!isSecure && this.httpsHelperBanner) {
        this.httpsHelperBanner.style.display = 'flex';
      }
    } catch (_) { }
  }

  copyMeetingLink() {
    let link = this.networkUrl || window.location.href;
    if ((window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1') && this.networkUrl) {
      link = this.networkUrl;
    }

    navigator.clipboard.writeText(link).then(() => {
      this.logTerminal(`Meeting link copied: ${link}`, 'success');
      if (this.btnHeaderCopyLink) {
        const origText = this.btnHeaderCopyLink.innerText;
        this.btnHeaderCopyLink.innerText = 'Copied';
        setTimeout(() => { this.btnHeaderCopyLink.innerText = origText; }, 2000);
      }
      if (this.btnCopyInviteLink) {
        this.btnCopyInviteLink.innerText = 'Copied';
        setTimeout(() => { this.btnCopyInviteLink.innerText = 'Copy Link'; }, 2000);
      }
    }).catch(() => {
      prompt('Meeting link:', link);
    });
  }

  // =========================================================================
  // PEER SYNC, ADMISSION & WEBRTC MESH
  // =========================================================================
  async initPeerSync() {
    await this.registerSelfPeer();
    this.pollPeers();
    setInterval(() => this.pollPeers(), 2000);
    setInterval(() => this.sendHeartbeat(), 3000);
    setInterval(() => this.pollConsensusStatus(), 1200);
    setInterval(() => this.pollSignals(), 1000);

    window.addEventListener('beforeunload', () => {
      try {
        navigator.sendBeacon('/api/peers/leave', JSON.stringify({ peer_id: this.myPeerId }));
      } catch (_) { }
    });
  }

  async registerSelfPeer() {
    try {
      const res = await fetch('/api/peers/join', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          peer_id: this.myPeerId,
          name: this.myDisplayName,
          avatar: this.myAvatar
        })
      });
      if (res.ok) {
        const data = await res.json();
        if (data.status === 'waiting') {
          this.userStatus = 'waiting';
          if (this.waitingModal) this.waitingModal.style.display = 'flex';
        } else {
          this.userStatus = 'admitted';
          if (this.waitingModal) this.waitingModal.style.display = 'none';
        }
      }
      this.logTerminal(`Joined room as: ${this.myDisplayName}`, 'success');
    } catch (_) { }
  }

  async sendHeartbeat() {
    try {
      await fetch('/api/peers/heartbeat', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ peer_id: this.myPeerId })
      });
    } catch (_) { }
  }

  async pollPeers() {
    try {
      const res = await fetch(`/api/peers?peer_id=${this.myPeerId}`);
      if (!res.ok) return;
      const data = await res.json();
      const peers = data.peers || [];

      if (data.auto_admit !== undefined && data.auto_admit !== this.autoAdmit) {
        this.autoAdmit = data.auto_admit;
        this.updateAutoAdmitUI();
      }

      if (this.peerCountVal) this.peerCountVal.textContent = peers.length;

      // Check self status (e.g. waiting room)
      const selfPeer = peers.find(p => p.is_self);
      if (selfPeer) {
        if (selfPeer.status === 'waiting') {
          if (this.waitingModal) this.waitingModal.style.display = 'flex';
          return;
        } else {
          if (this.waitingModal) this.waitingModal.style.display = 'none';
        }
      }

      // Check for pending guest knocking (for host)
      const waitingPeer = peers.find(p => !p.is_self && p.status === 'waiting');
      if (waitingPeer && this.admissionToast) {
        this.pendingPeerId = waitingPeer.id;
        if (this.admissionPeerName) this.admissionPeerName.textContent = waitingPeer.name || 'User';
        this.admissionToast.style.display = 'flex';
      } else if (this.admissionToast) {
        this.admissionToast.style.display = 'none';
      }

      // Reconcile WebRTC peer connections for admitted peers
      const admittedPeers = peers.filter(p => p.status !== 'waiting' && p.status !== 'denied');
      this.participants = admittedPeers.map(p => ({
        id: p.id,
        name: p.name || 'Participant',
        avatar: p.avatar || '',
        hasVoted: false,
        vote: null,
        isSelf: !!p.is_self
      }));

      for (const p of admittedPeers) {
        if (!p.is_self && !this.peerConnections.has(p.id)) {
          this.setupPeerConnection(p.id, p.name, p.avatar);
        }
      }

      // Cleanup disconnected peers
      const activeIds = new Set(admittedPeers.map(p => p.id));
      for (const [pid, pc] of this.peerConnections) {
        if (!activeIds.has(pid)) {
          pc.close();
          this.peerConnections.delete(pid);
          this.removeRemoteVideoCard(pid);
        }
      }
    } catch (_) { }
  }

  setupPeerConnection(remotePeerId, remoteName, remoteAvatar) {
    const rtcConfig = {
      iceServers: [
        { urls: 'stun:stun.l.google.com:19302' },
        { urls: 'stun:stun1.l.google.com:19302' },
        { urls: 'stun:stun2.l.google.com:19302' },
        { urls: 'stun:stun.cloudflare.com:3478' },
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

    // Add current active video track (screen or camera)
    const activeVideoTrack = (this.isScreenSharing && this.activeScreenTrack)
      ? this.activeScreenTrack
      : (this.localStream ? this.localStream.getVideoTracks()[0] : null);

    if (activeVideoTrack) {
      pc.addTrack(activeVideoTrack, this.localStream || new MediaStream([activeVideoTrack]));
    }

    if (this.localStream) {
      const audioTrack = this.localStream.getAudioTracks()[0];
      if (audioTrack) {
        pc.addTrack(audioTrack, this.localStream);
      }
    }

    pc.ontrack = (event) => {
      this.handleRemoteTrack(remotePeerId, remoteName, remoteAvatar, event);
    };

    pc.onicecandidate = (event) => {
      if (event.candidate) {
        this.sendSignal(remotePeerId, 'candidate', event.candidate);
      }
    };

    if (this.myPeerId < remotePeerId) {
      pc.onnegotiationneeded = async () => {
        try {
          const offer = await pc.createOffer();
          await pc.setLocalDescription(offer);
          this.sendSignal(remotePeerId, 'offer', offer);
        } catch (e) {
          console.warn('[WebRTC] Offer error:', e);
        }
      };
    }
  }

  async broadcastSignal(type, payload) {
    for (const peerId of this.peerConnections.keys()) {
      await this.sendSignal(peerId, type, payload);
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
    } catch (_) { }
  }

  async pollSignals() {
    try {
      const res = await fetch(`/api/signal/poll?peer_id=${this.myPeerId}`);
      if (!res.ok) return;
      const signals = await res.json();
      for (const sig of signals) {
        await this.handleIncomingSignal(sig);
      }
    } catch (_) { }
  }

  async handleIncomingSignal(sig) {
    if (sig.type === 'screen_state') {
      const state = sig.payload;
      if (state && state.peer_id) {
        const isSharing = !!state.sharing;
        this.handleRemoteScreenState(state.peer_id, isSharing);
      }
      return;
    }

    let pc = this.peerConnections.get(sig.from);
    if (!pc) {
      this.setupPeerConnection(sig.from, `User ${sig.from.substring(0, 6)}`, '');
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

  handleRemoteScreenState(peerId, isSharing) {
    const cardVideo = document.getElementById(`video-${peerId}`);
    const screenStatusPill = document.getElementById('screen-status-pill');

    if (isSharing && cardVideo && cardVideo.srcObject) {
      // Spotlight remote screen onto main stage
      this.screenVideo.srcObject = cardVideo.srcObject;
      this.screenVideo.style.display = 'block';
      this.localVideo.style.display = 'none';

      if (screenStatusPill) {
        screenStatusPill.textContent = 'Remote Screen Sharing';
        screenStatusPill.style.display = 'inline-flex';
      }
      this.logTerminal(`Remote peer ${peerId} shared their screen.`, 'success');
    } else if (!isSharing) {
      if (!this.isScreenSharing) {
        this.screenVideo.style.display = 'none';
        this.localVideo.style.display = 'block';
        if (screenStatusPill) screenStatusPill.style.display = 'none';
      }
    }
  }

  handleRemoteTrack(remotePeerId, remoteName, remoteAvatar, event) {
    const peersGrid = document.getElementById('peers-grid');
    if (!peersGrid) return;

    let card = document.getElementById(`peer-card-${remotePeerId}`);
    if (!card) {
      card = document.createElement('div');
      card.className = 'peer-card';
      card.id = `peer-card-${remotePeerId}`;
      const avatarHtml = remoteAvatar ? `<img src="${remoteAvatar}" class="peer-avatar-img" alt="Avatar">` : (remoteName || 'User').charAt(0).toUpperCase();

      card.innerHTML = `
        <div class="peer-video-wrapper">
          <video autoplay playsinline class="remote-video" id="video-${remotePeerId}" style="width:100%; height:100%; object-fit:cover; border-radius:12px;"></video>
          <div class="peer-avatar-fallback" id="avatar-${remotePeerId}" style="display:none;">${avatarHtml}</div>
        </div>
        <div class="peer-label">
          <span>${remoteName || remotePeerId}</span>
          <span class="peer-badge-live">Live</span>
        </div>
      `;
      peersGrid.appendChild(card);
    }

    const video = card.querySelector('video');
    if (video) {
      if (event.streams && event.streams[0]) {
        video.srcObject = event.streams[0];
      } else {
        if (!video.srcObject) video.srcObject = new MediaStream();
        video.srcObject.addTrack(event.track);
      }
    }
  }

  removeRemoteVideoCard(remotePeerId) {
    const card = document.getElementById(`peer-card-${remotePeerId}`);
    if (card) card.remove();
  }

  async fetchAndShowPeers() {
    try {
      const res = await fetch(`/api/peers?peer_id=${this.myPeerId}`);
      const data = await res.json();
      const peers = data.peers || [];
      if (this.peerCountVal) this.peerCountVal.textContent = peers.length;

      if (this.peersModalList) {
        this.peersModalList.innerHTML = peers.map((p) => `
          <div class="peer-list-row">
            <div>
              <div class="peer-row-name">${p.name || 'Participant'} ${p.is_self ? '<span style="color:#00f2fe; font-size:11px;">(You)</span>' : ''}</div>
              <div class="peer-row-id">ID: ${p.id}</div>
            </div>
            <div style="display:flex; align-items:center; gap:8px;">
              <span style="font-size:11px; color:#10b981;">Online</span>
            </div>
          </div>
        `).join('');
      }

      this.peersModal.style.display = 'flex';
      this.logTerminal(`Room: ${peers.length} active participant(s).`, 'info');
    } catch (e) {
      this.logTerminal(`Peers error: ${e.message}`, 'danger');
    }
  }

  // =========================================================================
  // CONSENSUS RECORDING & MID-CALL JOIN HANDLING
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

        // Requester does not need to vote on own ballot
        if (!this._votedBallots.has(b.ballot_id)) {
          if (b.requester_id === this.myPeerId) {
            this._votedBallots.add(b.ballot_id);
          } else {
            this.modalOverlay.style.display = 'flex';
          }
        }
      } else if (b.state === 'UnanimousOk' || (b.is_active && b.state !== 'Rejected' && b.state !== 'Revoked')) {
        // Late joining user proceeds with active recording without needing to consent
        this.modalOverlay.style.display = 'none';
        if (this.ballotState !== 'APPROVED') {
          this.ballotState = 'APPROVED';
          this.updateConsentGuard(true);
          if (this.recordingToast) this.recordingToast.classList.add('show');
          if (b.requester_id === this.myPeerId && !this.isRecording) {
            this.startLocalRecording();
          }
        }
      } else if (b.state === 'Rejected') {
        this.modalOverlay.style.display = 'none';
        if (this.ballotState === 'REQUESTED') {
          this.ballotState = 'REJECTED';
          this.updateConsentGuard(false);
          this.logTerminal('Recording blocked: A participant cast a veto.', 'danger');
        }
      } else if (b.state === 'Revoked') {
        this.modalOverlay.style.display = 'none';
        if (this.isRecording) {
          this.stopLocalRecording();
          this.ballotState = 'REVOKED';
          this.updateConsentGuard(false);
          this.logTerminal('Recording halted: Consent revoked.', 'danger');
        }
      }
    } catch (_) { }
  }

  async initiateRecordingConsent(type = 'av') {
    this.recordingType = type;
    this.logTerminal(`Initiating recording ballot (${type === 'audio' ? 'Audio-only' : 'Full AV'})...`, 'info');

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
          this.startLocalRecording();
        } else {
          this.modalOverlay.style.display = 'flex';
        }
      }
    } catch (e) {
      this.logTerminal(`Ballot error: ${e.message}`, 'danger');
    }
  }

  async castVote(isYes) {
    if (!this.currentBallot) return;
    const bid = this.currentBallot.ballotId;
    this._votedBallots.add(bid);

    try {
      await fetch('/api/consensus/vote', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          ballot_id: bid,
          voter_id: this.myPeerId,
          accept: isYes
        })
      });
      this.modalOverlay.style.display = 'none';
      if (!isYes) {
        this.ballotState = 'REJECTED';
        this.updateConsentGuard(false);
        this.logTerminal('Veto cast. Recording blocked.', 'danger');
      } else {
        this.logTerminal('Vote recorded: Agreed.', 'success');
      }
    } catch (e) {
      this.logTerminal(`Vote error: ${e.message}`, 'danger');
    }
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
    } catch (_) { }
    this.ballotState = 'REVOKED';
    this.updateConsentGuard(false);
    this.stopLocalRecording();
  }

  startLocalRecording() {
    if (this.ballotState !== 'APPROVED' || !window.__Grafana_consent_approved) {
      alert('Cannot record without unanimous consent.');
      return;
    }

    try {
      let streamToRecord;
      if (this.recordingType === 'audio') {
        const audioTracks = (this.localStream) ? this.localStream.getAudioTracks() : [];
        if (audioTracks.length === 0) {
          alert('No microphone track available for audio recording.');
          return;
        }
        streamToRecord = new MediaStream(audioTracks);
      } else {
        streamToRecord = this.screenStream || this.localStream;
      }

      if (!streamToRecord) {
        alert('No active media stream found.');
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
      const btnText = this.btnRecord.querySelector('.btn-text');
      if (btnText) btnText.textContent = 'Stop Rec';

      this.recordTimerInterval = setInterval(() => {
        const elapsed = Math.floor((Date.now() - this.recordStartTime) / 1000);
        const m = String(Math.floor(elapsed / 60)).padStart(2, '0');
        const s = String(elapsed % 60).padStart(2, '0');
        if (this.recTimer) this.recTimer.textContent = `${m}:${s}`;
      }, 1000);

      this.logTerminal(`Local recording active (${this.recordingType === 'audio' ? 'Audio' : 'Full AV'}). Zero cloud storage.`, 'success');
    } catch (err) {
      console.error('[Grafana] Failed to start MediaRecorder:', err);
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

    if (this.recordingToast) this.recordingToast.classList.remove('show');
    this.btnRecord.classList.remove('active');
    const btnText = this.btnRecord.querySelector('.btn-text');
    if (btnText) btnText.textContent = 'Record';
  }

  saveRecordingLocally() {
    if (this.recordedChunks.length === 0) return;

    const mime = (this.recordingType === 'audio') ? 'audio/webm' : 'video/webm';
    const ext = (this.recordingType === 'audio') ? 'weba' : 'webm';
    const blob = new Blob(this.recordedChunks, { type: mime });
    const url = URL.createObjectURL(blob);
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    const filename = `Grafana_local_${this.recordingType}_${timestamp}.${ext}`;

    const a = document.createElement('a');
    a.style.display = 'none';
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);

    setTimeout(() => URL.revokeObjectURL(url), 5000);
    this.recordedChunks = [];
    this.logTerminal(`Recording saved to disk: ${filename}`, 'success');
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

document.addEventListener('DOMContentLoaded', () => {
  window.GrafanaApp = new GrafanaWebClient();
});