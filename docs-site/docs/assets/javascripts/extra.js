// Crablet Documentation - Extra JS
document.addEventListener('DOMContentLoaded', function() {
  // Add copy button feedback
  const copyButtons = document.querySelectorAll('.md-clipboard');
  copyButtons.forEach(btn => {
    btn.addEventListener('click', () => {
      btn.classList.add('copied');
      setTimeout(() => btn.classList.remove('copied'), 2000);
    });
  });
});
