/* Feather phone client. Plain JavaScript, no framework, no build step. */
(() => {
  'use strict';

  const $ = (sel) => document.querySelector(sel);
  const TOKEN_KEY = 'feather.token';

  const state = {
    token: localStorage.getItem(TOKEN_KEY) || '',
    me: null,
    snap: null,
    selected: null,
    reply: null,
    actionMsg: null,
    picked: new Set(),
    mode: 'signup',
    search: '',
    lastTyping: 0,
    lastKey: '',
    polling: null,
    busy: false,
    installEvent: null,
    lastReadSent: {},
  };

  const el = {
    auth: $('#auth'), main: $('#main'), chatView: $('#chatView'),
    authTabs: $('#authTabs'), authForm: $('#authForm'), authName: $('#authName'),
    authPin: $('#authPin'), authError: $('#authError'), authSubmit: $('#authSubmit'),
    chatList: $('#chatList'), emptyList: $('#emptyList'), search: $('#search'),
    hubLine: $('#hubLine'), profileBtn: $('#profileBtn'), newChat: $('#newChat'),
    conn: $('#conn'), connText: $('#connText'),
    back: $('#back'), chatAvatar: $('#chatAvatar'), chatName: $('#chatName'),
    chatSub: $('#chatSub'), messages: $('#messages'), draft: $('#draft'),
    sendBtn: $('#sendBtn'), replyBar: $('#replyBar'), replyName: $('#replyName'),
    replyBody: $('#replyBody'), replyClose: $('#replyClose'), emojiBtn: $('#emojiBtn'),
    scrim: $('#scrim'), newSheet: $('#newSheet'), peopleList: $('#peopleList'),
    groupNameWrap: $('#groupNameWrap'), groupName: $('#groupName'), startChat: $('#startChat'),
    profileSheet: $('#profileSheet'), profileAvatar: $('#profileAvatar'),
    profileName: $('#profileName'), profileSpace: $('#profileSpace'),
    installBtn: $('#installBtn'), signOut: $('#signOut'),
    msgSheet: $('#msgSheet'), reactionRow: $('#reactionRow'),
    replyAction: $('#replyAction'), copyAction: $('#copyAction'), toast: $('#toast'),
    chatInfo: $('#chatInfo'),
  };

  const EMOJIS = ['👍', '❤️', '😂', '😮', '🙏', '🔥'];

  function esc(value) {
    return String(value == null ? '' : value)
      .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
  }

  function colorOf(n) {
    const v = Number(n) || 0x347868;
    return '#' + v.toString(16).padStart(6, '0').slice(-6);
  }

  function initials(name) {
    const bits = String(name || '?').trim().split(/\s+/).filter(Boolean);
    if (!bits.length) return '?';
    return (bits[0][0] + (bits[1] ? bits[1][0] : '')).toUpperCase();
  }

  function toast(text) {
    el.toast.textContent = text;
    el.toast.classList.add('show');
    clearTimeout(toast._t);
    toast._t = setTimeout(() => el.toast.classList.remove('show'), 2300);
  }

  async function api(path, options = {}) {
    const headers = { 'Content-Type': 'application/json' };
    if (state.token) headers.Authorization = 'Bearer ' + state.token;
    let res;
    try {
      res = await fetch(path, { ...options, headers });
    } catch (err) {
      console.error('Feather request failed', path, err);
      const e = new Error('Offline. Check the Wi-Fi and try again.');
      e.offline = true;
      throw e;
    }
    let data = {};
    try { data = await res.json(); } catch (err) { /* empty body */ }
    if (!res.ok) {
      const e = new Error(data.error || 'Something went wrong.');
      e.status = res.status;
      throw e;
    }
    return data;
  }

  // ---------------------------------------------------------------- auth
  function setAuthMode(mode) {
    state.mode = mode;
    document.querySelectorAll('#authTabs .seg').forEach((b) => {
      b.classList.toggle('active', b.dataset.mode === mode);
    });
    el.authSubmit.textContent = mode === 'signup' ? 'Create my account' : 'Sign in';
    el.authError.textContent = '';
  }

  el.authTabs.addEventListener('click', (e) => {
    const btn = e.target.closest('.seg');
    if (btn) setAuthMode(btn.dataset.mode);
  });

  el.authForm.addEventListener('submit', async (e) => {
    e.preventDefault();
    const name = el.authName.value.trim();
    const pin = el.authPin.value.trim();
    if (name.length < 2) { el.authError.textContent = 'Pick a name with at least 2 letters.'; return; }
    if (pin.length < 4) { el.authError.textContent = 'Your PIN needs at least 4 characters.'; return; }
    el.authSubmit.disabled = true;
    el.authError.textContent = '';
    try {
      const path = state.mode === 'signup' ? '/api/signup' : '/api/login';
      const data = await api(path, { method: 'POST', body: JSON.stringify({ name, pin }) });
      state.token = data.token;
      state.me = data.me;
      localStorage.setItem(TOKEN_KEY, data.token);
      el.authPin.value = '';
      await enterApp();
    } catch (err) {
      el.authError.textContent = err.message;
    } finally {
      el.authSubmit.disabled = false;
    }
  });

  async function enterApp() {
    await refresh(true);
    if (!state.token || !state.snap) return;
    el.auth.classList.add('hidden');
    el.chatView.classList.add('hidden');
    el.main.classList.remove('hidden');
    renderList();
    startPolling();
    if (!state.polling) startPolling();
  }

  function showAuth(message) {
    stopPolling();
    el.main.classList.add('hidden');
    el.chatView.classList.add('hidden');
    el.auth.classList.remove('hidden');
    if (message) el.authError.textContent = message;
  }

  function signOut() {
    localStorage.removeItem(TOKEN_KEY);
    state.token = '';
    state.me = null;
    state.snap = null;
    state.selected = null;
    stopPolling();
    showAuth('');
    history.replaceState(null, '', location.pathname);
  }

  // ---------------------------------------------------------------- data
  async function refresh(initial = false) {
    if (state.busy || !state.token) return;
    state.busy = true;
    try {
      const snap = await api('/api/state');
      state.snap = snap;
      if (snap.me) state.me = snap.me;
      hideConn();
      renderList();
      if (state.selected) {
        const conv = findConv(state.selected);
        if (!conv) {
          closeChat();
        } else {
          renderChatHeader(conv);
          renderMessages(conv, false);
          if (conv.unread > 0) markRead(conv);
        }
      }
      if (initial) {
        el.hubLine.textContent = (location.host || 'your space') + ' · your space';
        updateProfile();
      }
    } catch (err) {
      if (err.status === 401) {
        signOut();
        showAuth('Please sign in again.');
      } else if (!initial) {
        showConn(err.offline ? 'Offline — retrying…' : 'Reconnecting…');
      } else {
        throw err;
      }
    } finally {
      state.busy = false;
    }
  }

  function startPolling() {
    if (state.polling) return;
    state.polling = setInterval(() => {
      if (!document.hidden && state.token) refresh();
    }, 1600);
  }
  function stopPolling() {
    if (state.polling) clearInterval(state.polling);
    state.polling = null;
  }

  function showConn(text) {
    el.conn.classList.remove('hidden');
    el.connText.textContent = text;
  }
  function hideConn() { el.conn.classList.add('hidden'); }

  function findConv(id) {
    if (!state.snap) return null;
    return state.snap.convs.find((c) => c.id === Number(id)) || null;
  }
  function findPerson(id) {
    if (!state.snap) return null;
    return (state.snap.people || []).find((p) => p.id === id) || null;
  }

  // ---------------------------------------------------------------- list
  function convPreview(conv) {
    const msgs = conv.messages || [];
    if (!msgs.length) return 'Say hello';
    const last = msgs[msgs.length - 1];
    const mine = state.me && last.from === state.me.id;
    let prefix = '';
    if (mine) prefix = 'You: ';
    else if (conv.group) prefix = (last.from_name || 'Someone') + ': ';
    const text = last.text.replace(/\s+/g, ' ');
    return prefix + text;
  }

  function clock(d) {
    return String(d.getHours()).padStart(2, '0') + ':' + String(d.getMinutes()).padStart(2, '0');
  }

  function listTime(sec) {
    if (!sec) return '';
    const d = new Date(sec * 1000);
    const now = new Date();
    if (sameDay(d, now)) return clock(d);
    const y = new Date(now); y.setDate(now.getDate() - 1);
    if (sameDay(d, y)) return 'Yesterday';
    const diff = (now - d) / 86400000;
    if (diff < 7) return d.toLocaleDateString([], { weekday: 'short' });
    return d.toLocaleDateString([], { day: 'numeric', month: 'short' });
  }

  function sameDay(a, b) {
    return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
  }

  function renderList() {
    if (!state.snap) return;
    const q = state.search.trim().toLowerCase();
    const convs = state.snap.convs.filter((c) => {
      if (!q) return true;
      if (c.title.toLowerCase().includes(q)) return true;
      return (c.messages || []).some((m) => m.text.toLowerCase().includes(q));
    });

    el.chatList.innerHTML = convs.map((c) => {
      const last = (c.messages || [])[c.messages.length - 1];
      const mine = last && state.me && last.from === state.me.id;
      const tick = mine
        ? (last.read ? '<span class="tick">✓✓</span>' : '<span class="tick single">✓</span>')
        : '';
      const typing = (c.typing || []).length
        ? '<span class="chat-dot"></span><p>typing…</p>'
        : `<p>${esc(convPreview(c))}</p>`;
      const avatarColor = c.group
        ? 0x18a06e
        : (c.members.find((m) => !state.me || m.id !== state.me.id) || {}).color;
      return `
        <button class="chat-row" data-id="${c.id}">
          <span class="avatar" style="background:${colorOf(avatarColor)}">${esc(initials(c.title))}</span>
          <span class="chat-main">
            <span class="chat-line1">
              <span class="chat-name">${esc(c.title)}</span>
              <span class="chat-time">${esc(listTime(c.last_at))}</span>
            </span>
            <span class="chat-preview">
              ${typing}
              ${tick}
              ${c.unread > 0 ? `<span class="badge">${c.unread > 99 ? '99+' : c.unread}</span>` : ''}
            </span>
          </span>
        </button>`;
    }).join('');

    el.emptyList.classList.toggle('hidden', convs.length > 0 || !!q);
    if (!convs.length && q) {
      el.emptyList.querySelector('h2').textContent = 'Nothing found';
      el.emptyList.querySelector('p').textContent = 'Try another name or word.';
      el.emptyList.classList.remove('hidden');
    } else {
      el.emptyList.querySelector('h2').textContent = 'No chats yet';
      el.emptyList.querySelector('p').textContent = 'Start a conversation with someone in this space.';
    }
  }

  el.chatList.addEventListener('click', (e) => {
    const row = e.target.closest('.chat-row');
    if (row) openChat(Number(row.dataset.id));
  });

  el.search.addEventListener('input', () => {
    state.search = el.search.value;
    renderList();
  });

  // ---------------------------------------------------------------- chat
  function openChat(id) {
    const conv = findConv(id);
    if (!conv) return;
    state.selected = conv.id;
    state.reply = null;
    state.lastKey = '';
    el.main.classList.add('hidden');
    el.chatView.classList.remove('hidden');
    el.replyBar.classList.add('hidden');
    renderChatHeader(conv);
    renderMessages(conv, true);
    markRead(conv);
    updateHash(conv.id);
  }

  function closeChat() {
    state.selected = null;
    state.reply = null;
    el.chatView.classList.add('hidden');
    el.main.classList.remove('hidden');
    updateHash(null);
    renderList();
  }

  function updateHash(id) {
    const target = id ? '#chat-' + id : location.pathname;
    if (location.hash !== (id ? '#chat-' + id : '')) {
      history.pushState({ chat: id }, '', target);
    }
  }

  el.back.addEventListener('click', () => {
    if (history.state && history.state.chat) history.back();
    else closeChat();
  });

  window.addEventListener('popstate', (e) => {
    const id = e.state && e.state.chat;
    if (id) {
      const conv = findConv(id);
      if (conv) {
        state.selected = conv.id;
        el.main.classList.add('hidden');
        el.chatView.classList.remove('hidden');
        state.lastKey = '';
        renderChatHeader(conv);
        renderMessages(conv, true);
        markRead(conv);
      }
    } else {
      state.selected = null;
      el.chatView.classList.add('hidden');
      el.main.classList.remove('hidden');
      renderList();
    }
  });

  el.chatInfo.addEventListener('click', () => {
    const conv = findConv(state.selected);
    if (!conv) return;
    const names = conv.members.map((m) => m.name + (state.me && m.id === state.me.id ? ' (you)' : '')).join(', ');
    toast(names);
  });

  function renderChatHeader(conv) {
    el.chatName.textContent = conv.title;
    const other = conv.members.find((m) => !state.me || m.id !== state.me.id) || conv.members[0] || {};
    el.chatAvatar.textContent = initials(conv.title);
    el.chatAvatar.style.background = colorOf(conv.group ? 0x18a06e : other.color);
    const typing = (conv.typing || []);
    if (typing.length) {
      el.chatSub.textContent = typing.length === 1 ? typing[0] + ' is typing…' : 'Several people are typing…';
      el.chatSub.classList.add('typing');
    } else {
      el.chatSub.classList.remove('typing');
      if (conv.group) {
        const online = conv.members.filter((m) => m.online && (!state.me || m.id !== state.me.id)).length;
        el.chatSub.textContent = conv.members.length + ' members' + (online ? ' · ' + online + ' online' : '');
      } else {
        el.chatSub.textContent = other.online ? 'online' : 'in this space';
      }
    }
  }

  function dayLabel(sec) {
    const d = new Date(sec * 1000);
    const now = new Date();
    if (sameDay(d, now)) return 'Today';
    const y = new Date(now); y.setDate(now.getDate() - 1);
    if (sameDay(d, y)) return 'Yesterday';
    return d.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'short' });
  }

  function renderMessages(conv, force) {
    const list = conv.messages || [];
    const last = list[list.length - 1];
    const reactionCount = list.reduce((n, m) => n + (m.reactions || []).length, 0);
    const key = conv.id + ':' + list.length + ':' + (last ? last.id : 0) + ':' + reactionCount;
    if (!force && key === state.lastKey) return;
    state.lastKey = key;

    const nearBottom = el.messages.scrollHeight - el.messages.scrollTop - el.messages.clientHeight < 140;
    let html = '';
    let lastDay = '';
    let lastSender = '';
    for (const m of list) {
      const day = dayLabel(m.at);
      if (day !== lastDay) {
        html += `<div class="day"><span>${esc(day)}</span></div>`;
        lastDay = day;
        lastSender = '';
      }
      const mine = state.me && m.from === state.me.id;
      const showSender = conv.group && !mine && lastSender !== m.from;
      lastSender = m.from;

      const quote = m.reply_to
        ? `<div class="quote"><strong>${esc(m.reply_name || 'Reply')}</strong><span>${esc(m.reply_text || '')}</span></div>`
        : '';
      const groups = {};
      (m.reactions || []).forEach((r) => {
        groups[r.emoji] = groups[r.emoji] || { count: 0, mine: false };
        groups[r.emoji].count += 1;
        if (state.me && r.user === state.me.id) groups[r.emoji].mine = true;
      });
      const reactions = Object.keys(groups).length
        ? `<div class="reactions">${Object.entries(groups).map(([emoji, g]) => `<span class="reaction${g.mine ? ' mine' : ''}">${esc(emoji)}${g.count > 1 ? ' ' + g.count : ''}</span>`).join('')}</div>`
        : '';
      const ticks = mine
        ? `<span class="tick${m.read ? '' : ' single'}">${m.read ? '✓✓' : '✓'}</span>`
        : '';

      html += `
        <div class="msg-row ${mine ? 'out' : 'in'}">
          <div class="bubble" data-id="${m.id}">
            ${showSender ? `<p class="sender" style="color:${colorOf((findPerson(m.from) || {}).color)}">${esc(m.from_name)}</p>` : ''}
            ${quote}
            <div class="text">${esc(m.text)}</div>
            ${reactions}
            <div class="meta"><span class="time">${esc(clock(new Date(m.at * 1000)))}</span>${ticks}</div>
          </div>
        </div>`;
    }
    el.messages.innerHTML = html;
    if (force || nearBottom) el.messages.scrollTop = el.messages.scrollHeight;
  }

  async function markRead(conv) {
    if (!conv || !state.me) return;
    const list = conv.messages || [];
    const latest = list.length ? list[list.length - 1].id : 0;
    if (!latest || latest <= (state.lastReadSent[conv.id] || 0)) return;
    state.lastReadSent[conv.id] = latest;
    try {
      await api('/api/read', { method: 'POST', body: JSON.stringify({ conv: conv.id }) });
    } catch (err) { /* the next poll retries */ }
  }

  // ---------------------------------------------------------------- compose
  function autoGrow() {
    el.draft.style.height = 'auto';
    el.draft.style.height = Math.min(el.draft.scrollHeight, 120) + 'px';
  }

  el.draft.addEventListener('input', () => {
    autoGrow();
    const conv = findConv(state.selected);
    const text = el.draft.value.trim();
    if (!conv || !text) return;
    const now = Date.now();
    if (now - state.lastTyping > 1600) {
      state.lastTyping = now;
      api('/api/typing', { method: 'POST', body: JSON.stringify({ conv: conv.id }) }).catch(() => {});
    }
  });

  el.draft.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      sendMessage();
    }
  });

  el.sendBtn.addEventListener('click', sendMessage);

  async function sendMessage() {
    const conv = findConv(state.selected);
    const text = el.draft.value.trim();
    if (!conv || !text) return;
    const reply = state.reply;
    el.draft.value = '';
    autoGrow();
    state.reply = null;
    el.replyBar.classList.add('hidden');
    try {
      await api('/api/message', {
        method: 'POST',
        body: JSON.stringify({ conv: conv.id, text, reply_to: reply ? reply.id : null }),
      });
      state.lastKey = '';
      await refresh();
      const updated = findConv(conv.id);
      if (updated) {
        renderMessages(updated, true);
        renderChatHeader(updated);
      }
    } catch (err) {
      el.draft.value = text;
      autoGrow();
      state.reply = reply;
      if (reply) showReplyBar(reply);
      toast(err.message);
    }
  }

  function showReplyBar(msg) {
    el.replyName.textContent = msg.from_name || 'Reply';
    el.replyBody.textContent = msg.text;
    el.replyBar.classList.remove('hidden');
    el.draft.focus();
  }

  el.replyClose.addEventListener('click', () => {
    state.reply = null;
    el.replyBar.classList.add('hidden');
  });

  el.emojiBtn.addEventListener('click', () => {
    const emoji = EMOJIS[Math.floor(Math.random() * EMOJIS.length)];
    el.draft.value += emoji;
    autoGrow();
    el.draft.focus();
  });

  // ---------------------------------------------------------------- sheets
  function openSheet(sheet) {
    el.scrim.classList.remove('hidden');
    sheet.classList.remove('hidden');
  }
  function closeSheets() {
    el.scrim.classList.add('hidden');
    [el.newSheet, el.profileSheet, el.msgSheet].forEach((s) => s.classList.add('hidden'));
  }
  el.scrim.addEventListener('click', closeSheets);

  el.newChat.addEventListener('click', () => {
    state.picked.clear();
    el.groupName.value = '';
    renderPeople();
    openSheet(el.newSheet);
  });

  function renderPeople() {
    const people = (state.snap ? state.snap.people : []).filter((p) => !state.me || p.id !== state.me.id);
    el.peopleList.innerHTML = people.length ? people.map((p) => `
      <button class="person ${state.picked.has(p.id) ? 'picked' : ''}" data-id="${esc(p.id)}">
        <span class="avatar small" style="background:${colorOf(p.color)}">${esc(initials(p.name))}</span>
        <span class="pname">${esc(p.name)}${p.online ? ' · online' : ''}</span>
        <span class="check">${state.picked.has(p.id) ? '✓' : ''}</span>
      </button>`).join('') : '<p class="sheet-sub">No one else has joined this space yet. Share the hub address from your computer.</p>';
    el.groupNameWrap.classList.toggle('hidden', state.picked.size < 2);
    el.startChat.disabled = state.picked.size === 0;
    el.startChat.textContent = state.picked.size > 1 ? `Start group (${state.picked.size})` : 'Start chat';
  }

  el.peopleList.addEventListener('click', (e) => {
    const btn = e.target.closest('.person');
    if (!btn) return;
    const id = btn.dataset.id;
    if (state.picked.has(id)) state.picked.delete(id); else state.picked.add(id);
    renderPeople();
  });

  el.startChat.addEventListener('click', async () => {
    if (!state.picked.size) return;
    el.startChat.disabled = true;
    const payload = JSON.stringify({
      members: Array.from(state.picked),
      title: el.groupName.value.trim(),
    });
    try {
      let data;
      try {
        data = await api('/api/conversation', { method: 'POST', body: payload });
      } catch (err) {
        if (!err.offline) throw err;
        await new Promise((resolve) => setTimeout(resolve, 500));
        data = await api('/api/conversation', { method: 'POST', body: payload });
      }
      closeSheets();
      await refresh();
      if (data.id) openChat(data.id);
    } catch (err) {
      toast(err.message);
    } finally {
      el.startChat.disabled = false;
    }
  });

  el.profileBtn.addEventListener('click', () => {
    updateProfile();
    openSheet(el.profileSheet);
  });

  function updateProfile() {
    if (!state.me) return;
    el.profileBtn.textContent = initials(state.me.name);
    el.profileBtn.style.background = colorOf(state.me.color);
    el.profileAvatar.textContent = initials(state.me.name);
    el.profileAvatar.style.background = colorOf(state.me.color);
    el.profileName.textContent = state.me.name;
    el.profileSpace.textContent = (location.host || 'your space') + ' · your space';
    el.installBtn.classList.toggle('hidden', !state.installEvent);
  }

  el.signOut.addEventListener('click', () => {
    closeSheets();
    signOut();
  });

  window.addEventListener('beforeinstallprompt', (e) => {
    e.preventDefault();
    state.installEvent = e;
    el.installBtn.classList.remove('hidden');
  });

  el.installBtn.addEventListener('click', async () => {
    if (!state.installEvent) return;
    state.installEvent.prompt();
    try { await state.installEvent.userChoice; } catch (err) { /* dismissed */ }
    state.installEvent = null;
    el.installBtn.classList.add('hidden');
  });

  // message actions
  el.messages.addEventListener('click', (e) => {
    const bubble = e.target.closest('.bubble');
    if (!bubble) return;
    const conv = findConv(state.selected);
    const msg = conv && conv.messages.find((m) => m.id === Number(bubble.dataset.id));
    if (!msg) return;
    state.actionMsg = msg;
    openSheet(el.msgSheet);
  });

  el.reactionRow.innerHTML = EMOJIS.map((emoji) => `<button data-emoji="${esc(emoji)}">${esc(emoji)}</button>`).join('');

  el.reactionRow.addEventListener('click', async (e) => {
    const btn = e.target.closest('button');
    if (!btn || !state.actionMsg) return;
    try {
      await api('/api/react', {
        method: 'POST',
        body: JSON.stringify({ message: state.actionMsg.id, emoji: btn.dataset.emoji }),
      });
      closeSheets();
      await refresh();
    } catch (err) {
      toast(err.message);
    }
  });

  el.replyAction.addEventListener('click', () => {
    if (!state.actionMsg) return;
    state.reply = state.actionMsg;
    showReplyBar(state.actionMsg);
    closeSheets();
  });

  el.copyAction.addEventListener('click', async () => {
    if (!state.actionMsg) return;
    try {
      await navigator.clipboard.writeText(state.actionMsg.text);
      toast('Copied');
    } catch (err) {
      toast('Could not copy');
    }
    closeSheets();
  });

  // ---------------------------------------------------------------- boot
  function boot() {
    setAuthMode('signup');
    if (!state.token) {
      showAuth('');
    } else {
      enterApp().catch((err) => {
        if (err.status === 401) {
          localStorage.removeItem(TOKEN_KEY);
          state.token = '';
          showAuth('Please sign in again.');
        } else {
          showAuth(err.message);
        }
      });
    }

    if ('serviceWorker' in navigator) {
      navigator.serviceWorker.register('/sw.js').catch(() => {});
    }

    // Deep link into a chat when the page reloads on #chat-N.
    const m = /^#chat-(\d+)$/.exec(location.hash);
    if (m && state.token) {
      setTimeout(() => {
        const conv = findConv(Number(m[1]));
        if (conv) openChat(conv.id);
      }, 900);
    }

    document.addEventListener('visibilitychange', () => {
      if (!document.hidden && state.token) refresh();
    });
  }

  boot();
})();
