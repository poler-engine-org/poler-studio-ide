const doc = new Document({
sections: [
{ children: coverChildren },
{ children: ch1 }
// ... другие главы
]
});

Packer.toBuffer(doc).then(buffer => {
fs.writeFileSync("Eteria_Chronicles.docx", buffer);
});
