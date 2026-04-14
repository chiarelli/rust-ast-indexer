const { spawn } = require('child_process');
const path = require('path');

const INDEXER_BIN = path.join(__dirname, '..', '..', 'target', 'debug', 'rust_indexer');

function createIndexer() {
  return spawn(INDEXER_BIN, [], {
    stdio: ['pipe', 'pipe', 'pipe']
  });
}

function sendCommand(stdin, command) {
  stdin.write(JSON.stringify(command) + '\n');
}

function parseEvent(line) {
  try {
    return JSON.parse(line);
  } catch {
    return null;
  }
}

function main() {
  let targetPath = process.argv[2] || './src';
  if (!path.isAbsolute(targetPath)) {
    targetPath = path.resolve(__dirname, '..', '..', targetPath);
  }

  console.error('Starting indexer for:', targetPath);

  const indexer = createIndexer();
  const stdin = indexer.stdin;
  const stdout = indexer.stdout;
  const stderr = indexer.stderr;

  stderr.on('data', (data) => {
    console.error('[indexer stderr]:', data.toString().trim());
  });

  let capabilitiesEmitted = false;
  let chunks = [];

  stdout.on('line', (line) => {
    const event = parseEvent(line);
    if (!event) return;

    console.log('[event]:', JSON.stringify(event, null, 2));

    if (event.event === 'capabilities') {
      capabilitiesEmitted = true;
      console.log('\n=== Languages supported ===');
      const langs = event.payload?.languages || [];
      langs.forEach((lang) => {
        console.log(' -', lang);
      });
      console.log('=== End languages ===\n');

      console.log('Sending index_path command...');
      sendCommand(stdin, {
        protocol_version: '1.0.0',
        type: 'command',
        command: 'index_path',
        job_id: 'job-1',
        payload: {
          path: targetPath
        }
      });
    }

    if (event.event === 'chunk_emitted') {
      const chunk = event.payload;
      chunks.push({
        id: chunk.chunk_id,
        file: chunk.file,
        language: chunk.language,
        symbol_id: chunk.symbol_id,
        lines: `${chunk.start_line}-${chunk.end_line}`
      });
      console.log(`[chunk] ${chunk.file}:${chunk.start_line}-${chunk.end_line} (${chunk.language})`);
    }

    if (event.event === 'job_completed') {
      console.log('\n=== Job completed ===');
      console.log('Total chunks:', event.payload?.processed);
      console.log('Duration (ms):', event.payload?.duration_ms);

      if (chunks.length > 0) {
        console.log('\n=== Chunk summary ===');
        chunks.forEach((c, i) => {
          console.log(`${i + 1}. ${c.file}:${c.lines} [${c.language}] ${c.symbol_id}`);
        });
      }

      indexer.kill();
      process.exit(0);
    }

    if (event.event === 'error') {
      console.error('[error]', event.payload?.code, event.payload?.message);
      indexer.kill();
      process.exit(1);
    }
  });

  stdout.on('data', (data) => {
    const lines = data.toString().split('\n');
    lines.forEach((line) => {
      if (line.trim()) {
        stdout.emit('line', line);
      }
    });
  });

  indexer.on('error', (err) => {
    console.error('Failed to start indexer:', err.message);
    process.exit(1);
  });

  indexer.on('close', (code) => {
    if (code !== 0 && !capabilitiesEmitted) {
      console.error('Indexer exited with code', code);
      process.exit(1);
    }
  });

  console.log('Requesting list_languages...');
  sendCommand(stdin, {
    protocol_version: '1.0.0',
    type: 'command',
    command: 'list_languages'
  });
}

main();