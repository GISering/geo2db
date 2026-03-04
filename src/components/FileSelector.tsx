import { useCallback } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { Table, Card, Tag, Typography, Select, Space } from 'antd';
import { InboxOutlined, FileTextOutlined, FileAddOutlined, InfoCircleOutlined } from '@ant-design/icons';
import type { FileInfo, LayerInfo } from '../types';

const { Text } = Typography;

interface FileSelectorProps {
  files: FileInfo[];
  selectedFile: FileInfo | null;
  onSelectFile: (file: FileInfo) => void;
  onFilesSelected: (paths: string[]) => void;
  layers: LayerInfo[];
  selectedLayer: string | null;
  onSelectLayer: (layerName: string) => void;
}

const getFileIcon = (format: string) => {
  switch (format) {
    case 'Shapefile':
      return <FileTextOutlined style={{ fontSize: 24, color: '#1677ff' }} />;
    case 'GeoPackage':
      return <FileTextOutlined style={{ fontSize: 24, color: '#52c41a' }} />;
    case 'GeoJSON':
      return <FileTextOutlined style={{ fontSize: 24, color: '#722ed1' }} />;
    default:
      return <FileTextOutlined style={{ fontSize: 24 }} />;
  }
};

const getFormatColor = (format: string) => {
  switch (format) {
    case 'Shapefile':
      return 'blue';
    case 'GeoPackage':
      return 'green';
    case 'GeoJSON':
      return 'purple';
    default:
      return 'default';
  }
};

export function FileSelector({
  files,
  selectedFile,
  onSelectFile,
  onFilesSelected,
  layers,
  selectedLayer,
  onSelectLayer,
}: FileSelectorProps) {
  const handleBrowse = useCallback(async () => {
    const selected = await open({
      multiple: true,
      filters: [
        {
          name: '空间数据',
          extensions: ['shp', 'gpkg', 'geojson', 'kml'],
        },
      ],
    });

    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      onFilesSelected(paths);
    }
  }, [onFilesSelected]);

  const fileColumns = [
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
    },
  ];

  const fieldColumns = [
    {
      title: '字段名',
      dataIndex: 'name',
      key: 'name',
    },
    {
      title: '类型',
      dataIndex: 'field_type',
      key: 'field_type',
      render: (type: string) => <code>{type}</code>,
    },
  ];

  return (
    <Space direction="vertical" size="small" style={{ width: '100%' }}>
      {/* 文件选择区域 */}
      <Card
        size="small"
        title={
          <span>
            <FileAddOutlined style={{ marginRight: 8 }} />
            选择空间数据文件
          </span>
        }
      >
        <div
          style={{
            padding: '40px 20px',
            textAlign: 'center',
            cursor: 'pointer',
            border: '2px dashed #d9d9d9',
            borderRadius: 8,
            transition: 'border-color 0.3s',
          }}
          onClick={handleBrowse}
          onMouseEnter={(e) => (e.currentTarget.style.borderColor = '#1677ff')}
          onMouseLeave={(e) => (e.currentTarget.style.borderColor = '#d9d9d9')}
        >
          <p className="ant-upload-drag-icon">
            <InboxOutlined style={{ fontSize: 48, color: '#1677ff' }} />
          </p>
          <p className="ant-upload-text">点击选择文件</p>
          <p className="ant-upload-hint">
            支持 Shapefile (.shp)、GeoPackage (.gpkg)、GeoJSON (.geojson)、KML (.kml)
          </p>
        </div>
      </Card>

      {files.length > 0 && (
        <Card
          size="small"
          title={
            <span>
              <FileTextOutlined style={{ marginRight: 8 }} />
              已选择的文件
            </span>
          }
        >
          <Table
            dataSource={files}
            columns={fileColumns}
            rowKey="path"
            size="small"
            pagination={false}
            onRow={(record) => ({
              onClick: () => onSelectFile(record),
              style: {
                cursor: 'pointer',
                background: selectedFile?.path === record.path ? '#f0f5ff' : undefined,
              },
            })}
          />
        </Card>
      )}

      {selectedFile && (
        <Card
          size="small"
          title={
            <span>
              <InfoCircleOutlined style={{ marginRight: 8 }} />
              文件详情 - {selectedFile.name}
            </span>
          }
        >
          {layers.length > 1 && (
            <div style={{ marginBottom: 16 }}>
              <Text strong style={{ marginRight: 8 }}>选择图层:</Text>
              <Select
                value={selectedLayer}
                onChange={onSelectLayer}
                style={{ width: 250 }}
              >
                {layers.map((layer) => (
                  <Select.Option key={layer.name} value={layer.name}>
                    {layer.name} ({layer.feature_count} 个要素)
                  </Select.Option>
                ))}
              </Select>
            </div>
          )}

          <Table
            dataSource={selectedFile.fields}
            columns={fieldColumns}
            rowKey="name"
            size="small"
            pagination={false}
            title={() => '字段信息'}
          />
        </Card>
      )}
    </Space>
  );
}