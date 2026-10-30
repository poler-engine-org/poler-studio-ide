import * as fs from "fs";
import { Document, Packer, Paragraph, TextRun } from "docx";

const doc = new Document({
sections: [{
children: [
new Paragraph({
children: [
new TextRun("Hello World"),
new TextRun({ text: " - Bold text", bold: true }),
],
}),
],
}],
});

Packer.toBuffer(doc).then((buffer) => {
fs.writeFileSync("My Document.docx", buffer);
});

Этот код создаст файл My Document.docx с фразой "Hello World - Bold text"-
4
.
