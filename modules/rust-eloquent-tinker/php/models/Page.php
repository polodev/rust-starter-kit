<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Relations\HasMany;

class Page extends BaseModel
{
    protected $table = 'pages';

    protected $casts = ['is_published' => 'boolean'];

    public function translations(): HasMany
    {
        return $this->hasMany(PageTranslation::class, 'page_id');
    }

    /** The feature image rows (single-file collection "feature_image"). */
    public function featureImage()
    {
        return Media::where('model_type', 'page')->where('model_id', (string) $this->id)->where('collection_name', 'feature_image');
    }
}
