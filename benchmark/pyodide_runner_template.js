const { loadPyodide } = require("pyodide");
const fs = require("fs");

async function main() {
    let inputData = "";
    
    const chunks = [];
    process.stdin.on("data", chunk => chunks.push(chunk));
    
    await new Promise(resolve => process.stdin.on("end", resolve));
    inputData = Buffer.concat(chunks).toString();
    
    try {
        const pyodide = await loadPyodide();
        
        // 读取 Python 代码文件路径（从环境变量获取）
        const pythonCodePath = process.env.PYTHON_CODE_PATH;
        const pythonCode = fs.readFileSync(pythonCodePath, "utf8");
        
        // 设置 stdin - 使用字符串拼接避免模板字符串问题
        const stdinSetupCode = [
            "import sys",
            "import io",
            "",
            "class StdinWrapper:",
            "    def __init__(self, data):",
            "        self._buffer = io.StringIO(data)",
            "    def read(self):",
            "        return self._buffer.read()",
            "    def readline(self):",
            "        return self._buffer.readline()",
            "",
            "sys.stdin = StdinWrapper(" + JSON.stringify(inputData) + ")"
        ].join("\n");
        
        pyodide.runPython(stdinSetupCode);
        
        // 捕获 stdout
        pyodide.runPython([
            "import sys",
            "import io",
            "_stdout_buffer = io.StringIO()",
            "sys.stdout = _stdout_buffer"
        ].join("\n"));
        
        // 运行用户代码
        pyodide.runPython(pythonCode);
        
        // 获取输出
        const output = pyodide.runPython("_stdout_buffer.getvalue()");
        console.log(output);
        
    } catch (error) {
        console.error("Pyodide error:", error.message);
        process.exit(1);
    }
}

main();
