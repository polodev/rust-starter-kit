<?php

namespace App\Models;

/**
 * Spatie laravel-medialibrary's `media` table, shared with the Rust app. `model_type` holds a
 * short name here (`user`, `page`, `library`) or whatever your Laravel app stored.
 */
class Media extends BaseModel
{
    protected $table = 'media';

    protected $casts = [
        'manipulations' => 'array',
        'custom_properties' => 'array',
        'generated_conversions' => 'array',
        'responsive_images' => 'array',
    ];

    /** Where the file sits on its disk: `{id}/{file_name}`, unless it kept an older location. */
    public function getPathAttribute(): string
    {
        return $this->custom_properties['legacy_path'] ?? "{$this->id}/{$this->file_name}";
    }

    /** The address the Rust app serves it from. */
    public function getUrlAttribute(): string
    {
        return "/media/{$this->id}/{$this->file_name}";
    }

    public function getIsPublicAttribute(): bool
    {
        return in_array($this->custom_properties['public'] ?? false, [true, 1, '1', 'true'], true);
    }
}
