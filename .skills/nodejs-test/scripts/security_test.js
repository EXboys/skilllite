const readline = require('readline');
const fs = require('fs');
const path = require('path');

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false
});

let inputData = '';

rl.on('line', (line) => {
    inputData += line;
});

rl.on('close', () => {
    const results = {};
    
    // Test 1: Try to read /etc/passwd
    try {
        fs.readFileSync('/etc/passwd', 'utf8');
        results.read_passwd = 'ALLOWED';
    } catch (e) {
        results.read_passwd = e.message.includes('SKILLBOX') ? 'BLOCKED' : 'ERROR: ' + e.message;
    }
    
    // Test 2: Try to read home directory
    try {
        fs.readdirSync(process.env.HOME || '/Users');
        results.read_home = 'ALLOWED';
    } catch (e) {
        results.read_home = e.message.includes('SKILLBOX') ? 'BLOCKED' : 'ERROR: ' + e.message;
    }
    
    // Test 3: Try to write to /tmp
    try {
        fs.writeFileSync('/tmp/test_skillbox.txt', 'test');
        results.write_tmp = 'ALLOWED';
    } catch (e) {
        results.write_tmp = e.message.includes('SKILLBOX') ? 'BLOCKED' : 'ERROR: ' + e.message;
    }
    
    // Test 4: Try to execute child_process
    try {
        const { execSync } = require('child_process');
        execSync('echo test');
        results.child_process = 'ALLOWED';
    } catch (e) {
        results.child_process = e.message.includes('SKILLBOX') ? 'BLOCKED' : 'ERROR: ' + e.message;
    }
    
    // Test 5: Try to make HTTP request
    try {
        const http = require('http');
        http.get('http://example.com');
        results.http_request = 'ALLOWED';
    } catch (e) {
        results.http_request = e.message.includes('SKILLBOX') ? 'BLOCKED' : 'ERROR: ' + e.message;
    }
    
    console.log(JSON.stringify(results, null, 2));
});
