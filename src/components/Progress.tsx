import { Result, Progress, Button, Card, Statistic, Typography, Alert, List } from 'antd';
import { CheckCircleFilled, CloseCircleFilled, ReloadOutlined, LoadingOutlined, StopOutlined } from '@ant-design/icons';
import type { ImportResult, ImportProgress } from '../types';

const { Text } = Typography;

interface ProgressProps {
  isImporting: boolean;
  progress: ImportProgress | null;
  result: ImportResult | null;
  onReset: () => void;
  onCancel: () => void;
}

export function ProgressPanel({ isImporting, progress, result, onReset, onCancel }: ProgressProps) {
  if (isImporting) {
    const percent = progress ? Math.round((progress.current / progress.total) * 100) : 0;

    return (
      <Card>
        <Result
          icon={<LoadingOutlined style={{ color: '#1677ff' }} />}
          title="正在导入数据..."
          subTitle={progress?.message || '请稍候...'}
        >
          <div style={{ maxWidth: 400, margin: '0 auto' }}>
            <Progress
              percent={percent}
              status="active"
              showInfo
              strokeColor={{ from: '#108ee9', to: '#87d068' }}
            />
            <div style={{ textAlign: 'center', marginTop: 8 }}>
              <Text type="secondary">
                {progress?.current || 0} / {progress?.total || 0} 条记录
              </Text>
            </div>
            <div style={{ textAlign: 'center', marginTop: 16 }}>
              <Button danger icon={<StopOutlined />} onClick={onCancel}>
                取消导入
              </Button>
            </div>
          </div>
        </Result>
      </Card>
    );
  }

  if (result) {
    return (
      <Card>
        <Result
          icon={
            result.success ? (
              <CheckCircleFilled style={{ color: '#52c41a' }} />
            ) : (
              <CloseCircleFilled style={{ color: '#ff4d4f' }} />
            )
          }
          title={result.success ? '导入成功' : '导入完成（有错误）'}
          subTitle={result.success ? '数据已成功导入到数据库' : '部分数据导入失败，请查看错误日志'}
        >
          <div style={{ maxWidth: 600, margin: '0 auto' }}>
            <Card style={{ marginBottom: 16 }}>
              <div style={{ display: 'flex', justifyContent: 'space-around' }}>
                <Statistic
                  title="已导入"
                  value={result.imported_count}
                  valueStyle={{ color: '#52c41a' }}
                />
                <Statistic
                  title="失败"
                  value={result.error_count}
                  valueStyle={{ color: result.error_count > 0 ? '#ff4d4f' : undefined }}
                />
                <Statistic
                  title="耗时"
                  value={(result.duration_ms / 1000).toFixed(2)}
                  suffix="秒"
                />
              </div>
            </Card>

            {result.errors.length > 0 && (
              <Alert
                type="error"
                message={`${result.errors.length} 个错误`}
                description={
                  <List
                    size="small"
                    bordered
                    dataSource={result.errors.slice(0, 10)}
                    renderItem={(error) => (
                      <List.Item>
                        <Text type="danger" style={{ fontSize: 12 }}>{error}</Text>
                      </List.Item>
                    )}
                    style={{ maxHeight: 200, overflow: 'auto' }}
                  />
                }
                style={{ marginBottom: 16 }}
              />
            )}

            <Button type="primary" icon={<ReloadOutlined />} onClick={onReset} block>
              重新开始
            </Button>
          </div>
        </Result>
      </Card>
    );
  }

  return null;
}