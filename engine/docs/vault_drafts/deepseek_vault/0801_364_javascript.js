// Знайти всі посилання на файли на сторінці (зазвичай теги <a> з атрибутом href)
const links = Array.from(document.querySelectorAll('a[href*="/file/"], a[href*="/download/"]'));
links.forEach(link => {
const url = link.href;
  if (url && (url.includes('.pdf') || url.includes('.txt') || true)) { // змініть фільтр за потреби
const a = document.createElement('a');
a.href = url;
    a.download = ''; // спробувати примусове завантаження
document.body.appendChild(a);
a.click();
document.body.removeChild(a);
// затримка, щоб не перевантажити браузер
setTimeout(() => {}, 500);
}
});
