const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

async function loadDesignSystem() {
  console.log('检查并准备依赖...');

  let tailwind;
  try {
    tailwind = require('tailwindcss');
  } catch (e) {
    console.log('未检测到 tailwindcss，正在自动安装最新版...');
    execSync('npm install --no-save tailwindcss@latest', { stdio: 'inherit' });
    tailwind = require('tailwindcss');
  }

  console.log('正在加载 Tailwind CSS v4 设计系统...');

  if (typeof tailwind.__unstable__loadDesignSystem === 'function') {
    const tailwindDir = path.dirname(require.resolve('tailwindcss/package.json'));

    return await tailwind.__unstable__loadDesignSystem('@import "tailwindcss";', {
      loadStylesheet: async (id, base) => {
        let targetPath;
        if (id === 'tailwindcss' || id === 'tailwindcss/index.css') {
          targetPath = path.join(tailwindDir, 'index.css');
        } else if (id.startsWith('tailwindcss/')) {
          targetPath = path.join(tailwindDir, id.replace('tailwindcss/', ''));
        } else if (base) {
          targetPath = path.resolve(base, id);
        }

        if (targetPath && fs.existsSync(targetPath)) {
          const content = fs.readFileSync(targetPath, 'utf-8');
          return { content, base: path.dirname(targetPath) };
        }
        return { content: '', base };
      }
    });
  } else {
    throw new Error('未在 tailwindcss 中找到 __unstable__loadDesignSystem 方法，请确认安装的版本为 v4.x');
  }
}

module.exports = {
  loadDesignSystem,
};
