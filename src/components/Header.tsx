import { Steps } from 'antd';
import type { AppStep } from '../types';
import { UploadOutlined } from '@ant-design/icons';

interface HeaderProps {
  currentStep: AppStep;
}

const steps = [
  { title: '选择文件' },
  { title: '导入配置' },
  { title: '执行导入' },
];

export function Header() {
  return (
    <header className="app-header">
      <div className="app-title">
        <UploadOutlined className="app-icon" />
        <h1>空间数据导入工具</h1>
      </div>
    </header>
  );
}

export function StepBar({ currentStep }: HeaderProps) {
  const currentIndex = steps.findIndex((_, index) => {
    if (currentStep === 'files') return index === 0;
    if (currentStep === 'import') return index === 1;
    if (currentStep === 'progress') return index === 2;
    return 0;
  });

  return (
    <div className="step-bar">
      <Steps
        current={currentIndex}
        items={steps}
        size="small"
        className="header-steps"
      />
    </div>
  );
}