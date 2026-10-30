<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

/** Everything is mass assignable here: this is a console for the person who owns the database. */
abstract class BaseModel extends Model
{
    protected $guarded = [];
}
