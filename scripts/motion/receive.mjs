import http from "node:http";
import fs from "node:fs";
const output = process.argv[2] ?? "/tmp/keyturn-motion/renderer.json";
http.createServer((request, response) => {
  let body = "";
  request.on("data", chunk => { body += chunk; });
  request.on("end", () => {
    if (request.method === "POST") {
      const report = JSON.parse(body);
      fs.writeFileSync(output, JSON.stringify(report));
    }
    response.writeHead(200, { "Access-Control-Allow-Origin": "*", "Access-Control-Allow-Headers": "Content-Type" });
    response.end("ok");
  });
}).listen(55949, "127.0.0.1");
