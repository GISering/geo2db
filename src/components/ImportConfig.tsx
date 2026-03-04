import { useState, useEffect } from 'react';
import {
  Card,
  Form,
  Input,
  Select,
  Button,
  Radio,
  Typography,
  Divider,
  Space,
  Tag,
  Table,
  Flex,
} from 'antd';
import {
  DatabaseOutlined,
  SettingOutlined,
  FileTextOutlined,
  FileExcelOutlined,
  FilePdfOutlined,
  FileWordOutlined,
  FileOutlined,
  GlobalOutlined,
} from '@ant-design/icons';
import type { FileInfo, ImportMode, DbConfig, NamedDbConfig, ConnectionTestResult } from '../types';

const { Text } = Typography;

const getFileIcon = (format: string) => {
  const formatLower = format.toLowerCase();
  if (formatLower.includes('shp') || formatLower.includes('geojson') || formatLower.includes('gpkg') || formatLower.includes('kml')) {
    return <GlobalOutlined style={{ fontSize: 24, color: '#52c41a' }} />;
  }
  if (formatLower.includes('xlsx') || formatLower.includes('xls') || formatLower.includes('csv')) {
    return <FileExcelOutlined style={{ fontSize: 24, color: '#52c41a' }} />;
  }
  if (formatLower.includes('pdf')) {
    return <FilePdfOutlined style={{ fontSize: 24, color: '#ff4d4f' }} />;
  }
  if (formatLower.includes('doc') || formatLower.includes('docx')) {
    return <FileWordOutlined style={{ fontSize: 24, color: '#1890ff' }} />;
  }
  return <FileOutlined style={{ fontSize: 24, color: '#8c8c8c' }} />;
};

const getFormatColor = (format: string) => {
  const formatLower = format.toLowerCase();
  if (formatLower.includes('shp')) return 'geekblue';
  if (formatLower.includes('geojson')) return 'green';
  if (formatLower.includes('gpkg')) return 'purple';
  if (formatLower.includes('kml')) return 'orange';
  return 'default';
};

interface ImportConfigProps {
  onPrevStep: () => void;
  file: FileInfo | null;
  tableName: string;
  onTableNameChange: (name: string) => void;
  srs: string;
  onSrsChange: (srs: string) => void;
  importMode: ImportMode;
  onImportModeChange: (mode: ImportMode) => void;
  onStartImport: () => void;
  isImporting: boolean;
  dbConfig: DbConfig;
  configList: NamedDbConfig[];
  configName: string;
  onSelectDataSource: (name: string) => void;
  onAddDataSource: () => void;
  connectionResult: ConnectionTestResult | null;
  onTestConnection: () => Promise<any>;
  onTestConnectionForConfig: (config: DbConfig) => Promise<boolean>;
  onDeleteDataSource: (name: string) => Promise<boolean>;
  isTestingConnection: boolean;
}

const commonSrs = [
  { value: 'EPSG:4326', label: 'WGS84 (EPSG:4326)' },
  { value: 'EPSG:4490', label: 'CGCS2000 (EPSG:4490)' },
  { value: 'EPSG:3857', label: 'Web Mercator (EPSG:3857)' },
  { value: 'EPSG:32650', label: 'UTM Zone 50N (EPSG:32650)' },
];

export function ImportConfigPanel({
  onPrevStep,
  file,
  tableName,
  onTableNameChange,
  srs,
  onSrsChange,
  importMode,
  onImportModeChange,
  onStartImport,
  isImporting,
  dbConfig: _dbConfig,
  configList,
  configName,
  onSelectDataSource,
  onAddDataSource,
  connectionResult,
  onTestConnectionForConfig,
  onDeleteDataSource,
  isTestingConnection,
}: ImportConfigProps) {
  const [selectedDsName, setSelectedDsName] = useState('');

  // 同步 configName 到 selectedDsName
  useEffect(() => {
    setSelectedDsName(configName);
  }, [configName]);

  if (!file) {
    return (
      <div style={{ textAlign: 'center', padding: 24, color: '#999' }}>
        <Text>请先选择文件</Text>
      </div>
    );
  }

  const canStartImport = tableName && connectionResult?.success === true;

  return (
    <Space direction="vertical" size="small" style={{ width: '100%' }}>
      {/* 源文件信息卡片 */}
      <Card
        size="small"
        title={
          <span>
            <FileTextOutlined style={{ marginRight: 8 }} />
            源文件信息
          </span>
        }
      >
        <Table
          size="small"
          pagination={false}
          dataSource={[file]}
          rowKey="name"
          columns={[
            {
              title: '文件',
              dataIndex: 'name',
              key: 'name',
              render: (_: string, record: FileInfo) => (
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  {getFileIcon(record.format)}
                  <div>
                    <div style={{ fontWeight: 500 }}>{record.name}</div>
                    {record.srs && (
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        坐标系统: EPSG:{record.srs.epsg}
                      </Text>
                    )}
                  </div>
                </div>
              ),
            },
            {
              title: '格式',
              dataIndex: 'format',
              key: 'format',
              width: 100,
              render: (format: string) => (
                <Tag color={getFormatColor(format)}>{format}</Tag>
              ),
            },
            {
              title: '要素数量',
              dataIndex: 'feature_count',
              key: 'feature_count',
              width: 100,
              render: (count: number) => count.toLocaleString(),
            },
            {
              title: '几何类型',
              dataIndex: 'geometry_type',
              key: 'geometry_type',
              width: 120,
              render: (type: string) => <Tag color="purple">{type}</Tag>,
            },
          ]}
        />
      </Card>

      {/* 数据库配置卡片 */}
      <Card
        size="small"
        title={
          <span>
            <DatabaseOutlined style={{ marginRight: 8 }} />
            数据库配置
          </span>
        }
      >
        <Space direction="vertical" size="small" style={{ width: '100%' }}>
          {/* 数据源表格 */}
          <div style={{ marginBottom: 8 }}>
            <Flex justify="space-between" align="center" style={{ marginBottom: 8 }}>
              <Text type="secondary">数据源列表</Text>
              <Button onClick={onAddDataSource} size="small">新增数据源</Button>
            </Flex>
            <Table
              size="small"
              pagination={false}
              dataSource={configList}
              rowKey="name"
              columns={[
                {
                  title: '名称',
                  dataIndex: 'name',
                  key: 'name',
                  width: 150,
                },
                {
                  title: '类型',
                  dataIndex: 'db_type',
                  key: 'db_type',
                  width: 100,
                  render: (type: string) => <Tag color="blue">{type}</Tag>,
                },
                {
                  title: '地址',
                  key: 'address',
                  width: 180,
                  render: (_: any, record: NamedDbConfig) => (
                    <Text copyable>{record.host}:{record.port}</Text>
                  ),
                },
                {
                  title: '数据库',
                  dataIndex: 'database',
                  key: 'database',
                  width: 120,
                },
                {
                  title: '用户名',
                  dataIndex: 'username',
                  key: 'username',
                  width: 120,
                },
                {
                  title: '操作',
                  key: 'actions',
                  width: 200,
                  render: (_: any, record: NamedDbConfig) => (
                    <Space size="small">
                      <Button
                        size="small"
                        type={record.name === selectedDsName ? 'primary' : 'default'}
                        onClick={() => onSelectDataSource(record.name)}
                      >
                        选择
                      </Button>
                      <Button
                        size="small"
                        onClick={() => onTestConnectionForConfig(record)}
                        loading={isTestingConnection && record.name === selectedDsName}
                      >
                        测试
                      </Button>
                      <Button
                        size="small"
                        danger
                        onClick={() => onDeleteDataSource(record.name)}
                      >
                        删除
                      </Button>
                    </Space>
                  ),
                },
              ]}
            />
          </div>
        </Space>
      </Card>
      <Card
        size="small"
        title={
          <span>
            <SettingOutlined style={{ marginRight: 8 }} />
            导入设置
          </span>
        }
      >
        <Space direction="vertical" size="small" style={{ width: '100%' }}>
          {/* 导入设置表格 */}
          <Table
            size="small"
            pagination={false}
            dataSource={[{
              key: 'importSettings',
              database: selectedDsName ? (
                <Tag color="blue">{selectedDsName}</Tag>
              ) : (
                <Text type="secondary">未选择</Text>
              ),
              tableName: (
                <Input
                  value={tableName}
                  onChange={(e) => onTableNameChange(e.target.value)}
                  placeholder="输入表名"
                  style={{ width: 300 }}
                />
              ),
              srs: (
                <Select
                  value={srs}
                  onChange={onSrsChange}
                  style={{ width: 300 }}
                >
                  {commonSrs.map((s) => (
                    <Select.Option key={s.value} value={s.value}>
                      {s.label}
                    </Select.Option>
                  ))}
                </Select>
              ),
            }]}
            columns={[
              {
                title: '目标数据库',
                dataIndex: 'database',
                key: 'database',
                width: '30%',
              },
              {
                title: '目标表名',
                dataIndex: 'tableName',
                key: 'tableName',
                width: '35%',
              },
              {
                title: '目标坐标系',
                dataIndex: 'srs',
                key: 'srs',
                width: '35%',
              },
            ]}
          />

          <Divider style={{ margin: '8px 0' }} />

          {/* 导入模式 - 保持原有样式 */}
          <Form layout="inline">
            <Form.Item label={<Text type="secondary">导入模式</Text>}>
              <Radio.Group
                value={importMode}
                onChange={(e) => onImportModeChange(e.target.value)}
              >
                <Radio value="CreateNew">
                  <Text>创建新表</Text>
                </Radio>
                <Radio value="Append">
                  <Text>追加到现有表</Text>
                </Radio>
                <Radio value="Replace">
                  <Text>覆盖现有表</Text>
                </Radio>
              </Radio.Group>
            </Form.Item>
          </Form>
        </Space>
      </Card>

      {/* 底部按钮：上一步 + 开始导入 */}
      <div style={{ display: 'flex', gap: 16 }}>
        <Button onClick={onPrevStep} style={{ flex: 1 }}>
          上一步
        </Button>
        <Button
          type="primary"
          onClick={onStartImport}
          disabled={!canStartImport || isImporting}
          loading={isImporting}
          style={{ flex: 1 }}
        >
          {isImporting ? '导入中...' : '开始导入'}
        </Button>
      </div>
    </Space>
  );
}