from pathlib import Path

from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore",
    )

    cull_host: str = "127.0.0.1"
    cull_port: int = 8000
    cull_env: str = "development"
    cull_upload_dir: Path = Path("./.cull_data/uploads")

    postgres_dsn: str = (
        "postgresql://cull_user:cull_local_dev_password@localhost:5432/cull_workbench"
    )
    valkey_url: str = "valkey://localhost:6379/0"
    cache_ttl_seconds: int = 3600


settings = Settings()
