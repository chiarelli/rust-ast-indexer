// Minimal Node.js example to communicate with the indexer via JSONL over stdio
// Usage: node examples/nodejs/usage.js
const { spawn } = require('child_process');

const bin = './rust_indexer/target/debug/rust_indexer'; // adjust to built binary path
const p = spawn(bin, [], { stdio: ['pipe', 'pipe', 'inherit'] });

let buf = '';

p.stdout.on('data', chunk => {
  buf += String(chunk);
  let idx;
  while ((idx = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, idx).trim();
    buf = buf.slice(idx + 1);
    if (!line) continue;
    try {
      const msg = JSON.parse(line);
      // handle events
      if (msg.type === 'event') {
        console.log('EVENT', msg.event, msg.payload || '');
        if (msg.event === 'chunk_emitted') {
          console.log('chunk', msg.payload.chunk_id, 'from', msg.payload.file);
        }
        if (msg.event === 'error' && msg.payload && msg.payload.code === 'BACKPRESSURE') {
          console.warn('Backpressure: pausing. Will send resume when ready.');
          // Application-specific: process buffered items, then resume
          setTimeout(() => {
            const resume = { protocol_version: '1.0.0', type: 'command', command: 'resume', seq: 999 };
            p.stdin.write(JSON.stringify(resume) + '\n');
            console.log('Sent resume');
          }, 1000);
        }
        if (msg.event === 'file_invalid') {
          console.warn('Invalid file:', msg.payload && msg.payload.path);
        }
      }
      if (msg.type === 'ack') {
        console.log('ACK', msg.payload);
      }
    } catch (e) {
      console.error('Failed to parse line:', line);
    }
  }
});

p.on('exit', code => console.log('indexer exited', code));

// Example: ask capabilities
const listLanguages = { protocol_version: '1.0.0', type: 'command', command: 'list_languages', seq: 1 };
p.stdin.write(JSON.stringify(listLanguages) + '\n');

// Example: start an indexing job (adjust path)
const indexPathCmd = {
  protocol_version: '1.0.0',
  type: 'command',
  command: 'index_path',
  seq: 2,
  job_id: 'job-001',
  payload: {
    path: '.',
    language: 'rust',
    ignore_patterns: ['target/**'],
    options: { max_concurrency: 4, chunk_lines: 200, backpressure: { max_queue_size: 500, ack_required: false } }
  }
};

// send the command
p.stdin.write(JSON.stringify(indexPathCmd) + '\n');

// Example: incremental index using git diffs
const incremental = {
  protocol_version: '1.0.0',
  type: 'command',
  command: 'incremental_index',
  seq: 3,
  job_id: 'job-002',
  payload: { path: '.', use_git: true, git_range: { from: 'HEAD~1', to: 'HEAD' }, options: { max_concurrency: 2 } }
};
// p.stdin.write(JSON.stringify(incremental) + '\n');

console.log('Commands sent. Waiting for events...');
